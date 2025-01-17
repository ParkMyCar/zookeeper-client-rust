use bytes::{Buf, BufMut};
use ignore_result::Ignore;
use tokio::sync::oneshot;

use super::types::WatchMode;
use super::watch::WatchReceiver;
use crate::error::Error;
use crate::proto::{self, AddWatchMode, ConnectRequest, OpCode, RequestHeader};
use crate::record::{self, Record, StaticRecord};

impl MarshalledRequest {
    pub fn new_request(code: OpCode, body: &dyn Record) -> MarshalledRequest {
        let header = RequestHeader::with_code(code);
        let buf = proto::build_session_request(&header, body);
        MarshalledRequest(buf)
    }

    pub fn new_record(body: &dyn Record) -> MarshalledRequest {
        MarshalledRequest(proto::build_record_request(body))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn get_code(&self) -> OpCode {
        let mut buf = &self.0[8..12];
        buf.get_i32().try_into().unwrap()
    }

    pub fn get_xid(&self) -> i32 {
        let mut xid_buf = &self.0[4..8];
        xid_buf.get_i32()
    }

    pub fn set_xid(&mut self, xid: i32) {
        let mut xid_buf = &mut self.0[4..8];
        xid_buf.put_i32(xid);
    }

    pub fn get_operation_info(&self) -> (OpCode, Option<(&str, WatchMode)>) {
        let op_code = self.get_code();
        let watcher_info = match op_code {
            OpCode::GetData
            | OpCode::Exists
            | OpCode::GetChildren
            | OpCode::GetChildren2
            | OpCode::AddWatch
            | OpCode::RemoveWatches => {
                let offset = 4 + RequestHeader::record_len();
                let mut body = &self.0[offset..];
                let server_path = record::deserialize::<&str>(&mut body).unwrap();
                if op_code == OpCode::AddWatch || op_code == OpCode::RemoveWatches {
                    body.advance(3);
                }
                let watch = body.get_u8();
                if op_code == OpCode::AddWatch {
                    let add_mode = AddWatchMode::try_from(watch as i32).unwrap();
                    Some((server_path, WatchMode::from(add_mode)))
                } else if op_code == OpCode::RemoveWatches {
                    Some((server_path, WatchMode::try_from(watch as i32).unwrap()))
                } else if watch == 1 {
                    let mode = if op_code == OpCode::GetData || op_code == OpCode::Exists {
                        WatchMode::Data
                    } else {
                        WatchMode::Child
                    };
                    Some((server_path, mode))
                } else {
                    assert!(watch == 0);
                    None
                }
            },
            _ => None,
        };
        (op_code, watcher_info)
    }
}

pub enum Operation {
    Connect(ConnectOperation),
    Auth(AuthOperation),
    Session(SessionOperation),
}

impl Operation {
    pub fn get_data(&self) -> &[u8] {
        match self {
            Operation::Connect(operation) => operation.request.as_slice(),
            Operation::Session(operation) => operation.request.as_slice(),
            Operation::Auth(operation) => operation.request.as_slice(),
        }
    }
}

pub struct ConnectOperation {
    pub request: Vec<u8>,
}

pub struct AuthOperation {
    pub request: MarshalledRequest,
}

#[derive(Debug)]
pub struct MarshalledRequest(pub Vec<u8>);

#[derive(Debug)]
pub struct SessionOperation {
    pub request: MarshalledRequest,
    pub responser: StateResponser,
}

pub type StateReceiver = oneshot::Receiver<Result<(Vec<u8>, WatchReceiver), Error>>;
type StateSender = oneshot::Sender<Result<(Vec<u8>, WatchReceiver), Error>>;

#[derive(Default, Debug)]
pub struct StateResponser(Option<StateSender>);

impl StateResponser {
    pub fn new(sender: oneshot::Sender<Result<(Vec<u8>, WatchReceiver), Error>>) -> Self {
        StateResponser(Some(sender))
    }

    pub fn none() -> Self {
        StateResponser(None)
    }

    pub fn send(mut self, result: Result<(Vec<u8>, WatchReceiver), Error>) {
        if let Some(sender) = self.0.take() {
            sender.send(result).ignore();
        }
    }

    pub fn send_empty(self) {
        self.send(Ok((Vec::new(), WatchReceiver::None)));
    }
}

pub fn build_connect_operation(request: &ConnectRequest) -> ConnectOperation {
    let buf = proto::build_record_request(request);
    ConnectOperation { request: buf }
}

pub fn build_auth_operation(code: OpCode, body: &dyn Record) -> AuthOperation {
    let request = MarshalledRequest::new_request(code, body);
    AuthOperation { request }
}

pub fn build_state_operation(code: OpCode, body: &dyn Record) -> (SessionOperation, StateReceiver) {
    let request = MarshalledRequest::new_request(code, body);
    let (sender, receiver) = oneshot::channel();
    let operation = SessionOperation { request, responser: StateResponser::new(sender) };
    (operation, receiver)
}

pub fn build_session_operation(request: &dyn Record) -> SessionOperation {
    let request = MarshalledRequest::new_record(request);
    SessionOperation { request, responser: StateResponser::default() }
}
