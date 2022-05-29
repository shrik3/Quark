// Copyright (c) 2021 Quark Container Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]
#![allow(bare_trait_objects)]
#![feature(map_first_last)]
#![allow(non_camel_case_types)]
#![feature(llvm_asm)]
#![allow(deprecated)]
#![feature(thread_id_value)]
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate bit_field;
extern crate core_affinity;
extern crate errno;

#[macro_use]
extern crate serde_derive;
extern crate cache_padded;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate scopeguard;

#[macro_use]
extern crate lazy_static;

extern crate libc;
extern crate spin;
extern crate x86_64;
#[macro_use]
extern crate log;
extern crate caps;
extern crate fs2;
extern crate regex;
extern crate simplelog;
extern crate tabwriter;

#[macro_use]
pub mod print;

#[macro_use]
pub mod asm;
pub mod kernel_def;
pub mod qlib;

pub mod common;
pub mod rdma_def;
// pub mod rdma_svc_cli;
pub mod unix_socket_def;

use self::qlib::ShareSpaceRef;
use alloc::slice;
use alloc::sync::Arc;
use fs2::FileExt;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::io::Error;
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
pub static SHARE_SPACE: ShareSpaceRef = ShareSpaceRef::New();
use crate::qlib::rdma_share::*;
use common::EpollEvent;
use common::*;
use local_ip_address::list_afinet_netifas;
use local_ip_address::local_ip;
use qlib::linux_def::*;
use qlib::socket_buf::SocketBuff;
use qlib::rdma_svc_cli::*;
use spin::{Mutex, MutexGuard};
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::{env, mem, ptr, thread, time};
use qlib::unix_socket::UnixSocket;

fn main() -> io::Result<()> {
    let mut fds: HashMap<i32, FdType> = HashMap::new();
    let gatewayCli: GatewayClient;
    gatewayCli = GatewayClient::initialize("/tmp/rdma_srv");

    let cliEventFd = gatewayCli.rdmaSvcCli.cliEventFd;
    unblock_fd(cliEventFd);
    unblock_fd(gatewayCli.rdmaSvcCli.srvEventFd);

    let epoll_fd = epoll_create().expect("can create epoll queue");
    epoll_add(epoll_fd, cliEventFd, read_event(cliEventFd as u64))?;
    fds.insert(cliEventFd, FdType::ClientEvent);

    //Bind all registered port
    //100733100, 58433
    //134654144
    let serverSockFd = gatewayCli.sockIdMgr.lock().AllocId().unwrap();
    let _ret = gatewayCli.bind(
        serverSockFd,
        u32::from(Ipv4Addr::from_str("192.168.6.8").unwrap()).to_be(),
        16868u16.to_be(),
    );

    let _ret = gatewayCli.listen(serverSockFd, 5);
    gatewayCli
        .rdmaSvcCli
        .cliShareRegion
        .lock()
        .clientBitmap
        .store(1, Ordering::SeqCst);
    wait(epoll_fd, &gatewayCli, &mut fds);

    return Ok(());
}

fn wait(epoll_fd: i32, gatewayCli: &GatewayClient, fds: &mut HashMap<i32, FdType>) {
    let mut events: Vec<EpollEvent> = Vec::with_capacity(1024);

    // mapping between sockfd maintained by rdmaSvcCli and fd for connecting to external server.
    let mut sockFdMappings: HashMap<u32, i32> = HashMap::new();
    loop {
        events.clear();
        {
            gatewayCli
                .rdmaSvcCli
                .cliShareRegion
                .lock()
                .clientBitmap
                .store(1, Ordering::SeqCst);
        }
        let res = match syscall!(epoll_wait(
            epoll_fd,
            events.as_mut_ptr() as *mut libc::epoll_event,
            1024,
            -1 as libc::c_int,
        )) {
            Ok(v) => v,
            Err(e) => panic!("error during epoll wait: {}", e),
        };

        unsafe { events.set_len(res as usize) };

        for ev in &events {
            let event_data = fds.get(&(ev.U64 as i32));
            match event_data {
                Some(FdType::TCPSocketServer(_port)) => {
                    println!("Egress gateway doesn't have this type!");
                }
                Some(FdType::TCPSocketConnect(sockfd)) => {
                    let mut sockInfo;
                    {
                        let mut sockFdInfos = gatewayCli.dataSockFdInfos.lock();
                        sockInfo = sockFdInfos.get_mut(sockfd).unwrap().clone();
                    }

                    if ev.Events & EVENT_IN as u32 != 0 {
                        gatewayCli.ReadFromSocket(&mut sockInfo, &sockFdMappings);
                    }
                    if ev.Events & EVENT_OUT as u32 != 0 {
                        gatewayCli.WriteToSocket(&mut sockInfo, &sockFdMappings);
                    }
                }
                Some(FdType::ClientEvent) => {
                    loop {
                        let request = gatewayCli.rdmaSvcCli.cliShareRegion.lock().cq.Pop();
                        match request {
                            Some(cq) => match cq.msg {
                                RDMARespMsg::RDMAConnect(response) => {
                                    let ioBufIndex = response.ioBufIndex as usize;
                                    let mut sockFdInfos = gatewayCli.dataSockFdInfos.lock();
                                    let sockInfo = sockFdInfos.get_mut(&response.sockfd).unwrap();
                                    {
                                        let shareRegion =
                                            gatewayCli.rdmaSvcCli.cliShareRegion.lock();
                                        let sockInfo = DataSock::New(
                                            sockInfo.fd, //Allocate fd
                                            sockInfo.srcIpAddr,
                                            sockInfo.srcPort,
                                            sockInfo.dstIpAddr,
                                            sockInfo.dstPort,
                                            SockStatus::ESTABLISHED,
                                            response.channelId,
                                            Arc::new(SocketBuff::InitWithShareMemory(
                                                MemoryDef::DEFAULT_BUF_PAGE_COUNT,
                                                &shareRegion.ioMetas[ioBufIndex].readBufAtoms
                                                    as *const _
                                                    as u64,
                                                &shareRegion.ioMetas[ioBufIndex].writeBufAtoms
                                                    as *const _
                                                    as u64,
                                                &shareRegion.ioMetas[ioBufIndex].consumeReadData
                                                    as *const _
                                                    as u64,
                                                &shareRegion.iobufs[ioBufIndex].read as *const _
                                                    as u64,
                                                &shareRegion.iobufs[ioBufIndex].write as *const _
                                                    as u64,
                                            )),
                                        );
                                        sockFdInfos.insert(sockInfo.fd, sockInfo);
                                    }
                                    let sockInfo = sockFdInfos.get_mut(&response.sockfd).unwrap();
                                    gatewayCli
                                        .channelToSockInfos
                                        .lock()
                                        .insert(response.channelId, sockInfo.clone());

                                    gatewayCli.ReadFromSocket(sockInfo, &sockFdMappings);
                                }
                                RDMARespMsg::RDMAAccept(response) => {
                                    let mut sockFdInfos = gatewayCli.serverSockFdInfos.lock();
                                    let sockInfo = sockFdInfos.get_mut(&response.sockfd).unwrap();

                                    let ioBufIndex = response.ioBufIndex as usize;
                                    let dataSockFd = gatewayCli.sockIdMgr.lock().AllocId().unwrap();
                                    let shareRegion = gatewayCli.rdmaSvcCli.cliShareRegion.lock();
                                    let dataSockInfo = DataSock::New(
                                        dataSockFd, //Allocate fd
                                        sockInfo.srcIpAddr,
                                        sockInfo.srcPort,
                                        response.dstIpAddr,
                                        response.dstPort,
                                        SockStatus::ESTABLISHED,
                                        response.channelId,
                                        Arc::new(SocketBuff::InitWithShareMemory(
                                            MemoryDef::DEFAULT_BUF_PAGE_COUNT,
                                            &shareRegion.ioMetas[ioBufIndex].readBufAtoms
                                                as *const _
                                                as u64,
                                            &shareRegion.ioMetas[ioBufIndex].writeBufAtoms
                                                as *const _
                                                as u64,
                                            &shareRegion.ioMetas[ioBufIndex].consumeReadData
                                                as *const _
                                                as u64,
                                            &shareRegion.iobufs[ioBufIndex].read as *const _ as u64,
                                            &shareRegion.iobufs[ioBufIndex].write as *const _
                                                as u64,
                                        )),
                                    );
                                    // println!("RDMARespMsg::RDMAAccept, sockfd: {}, channelId: {}", dataSockFd, response.channelId);

                                    gatewayCli
                                        .dataSockFdInfos
                                        .lock()
                                        .insert(dataSockFd, dataSockInfo.clone());
                                    sockInfo.acceptQueue.lock().EnqSocket(dataSockFd);
                                    gatewayCli
                                        .channelToSockInfos
                                        .lock()
                                        .insert(response.channelId, dataSockInfo.clone());

                                    let sock_fd = unsafe {
                                        libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0)
                                    };
                                    unblock_fd(sock_fd);
                                    fds.insert(sock_fd, FdType::TCPSocketConnect(dataSockFd));
                                    let _ret = epoll_add(
                                        epoll_fd,
                                        sock_fd,
                                        read_write_event(sock_fd as u64),
                                    );

                                    unsafe {
                                        //TODO: this should be use control plane data: egressPort -> (ipAddr, port)
                                        let serv_addr: libc::sockaddr_in = libc::sockaddr_in {
                                            sin_family: libc::AF_INET as u16,
                                            sin_port: 25028u16.to_be(),
                                            sin_addr: libc::in_addr {
                                                s_addr: u32::from(
                                                    // Ipv4Addr::from_str("172.16.1.6").unwrap(),
                                                    Ipv4Addr::from_str("127.0.0.1").unwrap(),
                                                )
                                                .to_be(),
                                            },
                                            sin_zero: mem::zeroed(),
                                        };
                                        let _ret = libc::connect(
                                            sock_fd,
                                            &serv_addr as *const libc::sockaddr_in
                                                as *const libc::sockaddr,
                                            mem::size_of_val(&serv_addr) as u32,
                                        );
                                        // if ret < 0 {
                                        //     println!(
                                        //         "ret is {}, error: {}",
                                        //         ret,
                                        //         Error::last_os_error()
                                        //     );
                                        // }
                                    }
                                    sockFdMappings.insert(dataSockFd, sock_fd);
                                }
                                RDMARespMsg::RDMANotify(response) => {
                                    if response.event & EVENT_IN != 0 {
                                        let mut channelToSockInfos =
                                            gatewayCli.channelToSockInfos.lock();
                                        let sockInfo = channelToSockInfos
                                            .get_mut(&response.channelId)
                                            .unwrap();
                                        gatewayCli.WriteToSocket(sockInfo, &sockFdMappings);
                                    }
                                    if response.event & EVENT_OUT != 0 {
                                        let mut sockInfo;
                                        {
                                            let mut channelToSockInfos =
                                                gatewayCli.channelToSockInfos.lock();
                                            sockInfo = channelToSockInfos
                                                .get_mut(&response.channelId)
                                                .unwrap()
                                                .clone();
                                        }

                                        gatewayCli.ReadFromSocket(&mut sockInfo, &sockFdMappings);
                                    }
                                }
                                RDMARespMsg::RDMAFinNotify(response) => {
                                    let mut channelToSockInfos =
                                        gatewayCli.channelToSockInfos.lock();
                                    let sockInfo =
                                        channelToSockInfos.get_mut(&response.channelId).unwrap();
                                    if response.event & FIN_RECEIVED_FROM_PEER != 0 {
                                        *sockInfo.finReceived.lock() = true;
                                        gatewayCli.WriteToSocket(sockInfo, &sockFdMappings);
                                    }
                                }
                            },
                            None => {
                                break;
                            }
                        }
                    }
                }
                None => {}
            }
        }
    }
}