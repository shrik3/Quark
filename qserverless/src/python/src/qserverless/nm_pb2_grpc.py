# Generated by the gRPC Python protocol compiler plugin. DO NOT EDIT!
"""Client and server classes corresponding to protobuf-defined services."""
import grpc

import nm_pb2 as nm__pb2


class NodeAgentServiceStub(object):
    """Missing associated documentation comment in .proto file."""

    def __init__(self, channel):
        """Constructor.

        Args:
            channel: A grpc.Channel.
        """
        self.StreamProcess = channel.stream_stream(
                '/nm.NodeAgentService/StreamProcess',
                request_serializer=nm__pb2.NodeAgentRespMsg.SerializeToString,
                response_deserializer=nm__pb2.NodeAgentReq.FromString,
                )


class NodeAgentServiceServicer(object):
    """Missing associated documentation comment in .proto file."""

    def StreamProcess(self, request_iterator, context):
        """Missing associated documentation comment in .proto file."""
        context.set_code(grpc.StatusCode.UNIMPLEMENTED)
        context.set_details('Method not implemented!')
        raise NotImplementedError('Method not implemented!')


def add_NodeAgentServiceServicer_to_server(servicer, server):
    rpc_method_handlers = {
            'StreamProcess': grpc.stream_stream_rpc_method_handler(
                    servicer.StreamProcess,
                    request_deserializer=nm__pb2.NodeAgentRespMsg.FromString,
                    response_serializer=nm__pb2.NodeAgentReq.SerializeToString,
            ),
    }
    generic_handler = grpc.method_handlers_generic_handler(
            'nm.NodeAgentService', rpc_method_handlers)
    server.add_generic_rpc_handlers((generic_handler,))


 # This class is part of an EXPERIMENTAL API.
class NodeAgentService(object):
    """Missing associated documentation comment in .proto file."""

    @staticmethod
    def StreamProcess(request_iterator,
            target,
            options=(),
            channel_credentials=None,
            call_credentials=None,
            insecure=False,
            compression=None,
            wait_for_ready=None,
            timeout=None,
            metadata=None):
        return grpc.experimental.stream_stream(request_iterator, target, '/nm.NodeAgentService/StreamProcess',
            nm__pb2.NodeAgentRespMsg.SerializeToString,
            nm__pb2.NodeAgentReq.FromString,
            options, channel_credentials,
            insecure, call_credentials, compression, wait_for_ready, timeout, metadata)
