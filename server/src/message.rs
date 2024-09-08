use std::{
    io::{self, Read},
    task::Poll,
};

use pond_deployment::{DeploymentLogs, LogStream};
use rocket::{
    response::Responder,
    tokio::{
        io::{AsyncRead, ReadBuf},
        task::block_in_place,
    },
    Response,
};

pub struct AsyncLogStream {
    info: Option<LogStream>,
    error: Option<LogStream>,
}

impl AsyncLogStream {
    pub fn from_deployment_logs(deployment_logs: DeploymentLogs) -> Self {
        let (info, error) = deployment_logs.into_read();
        Self {
            info: Some(info),
            error: Some(error),
        }
    }
}

impl<'r> Responder<'r, 'r> for AsyncLogStream {
    fn respond_to(self, _request: &'r rocket::Request<'_>) -> rocket::response::Result<'r> {
        Ok(Response::build().streamed_body(self).finalize())
    }
}

impl AsyncRead for AsyncLogStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        block_in_place(move || match &mut self.get_mut().info {
            Some(r) => {
                let bytes_read = r.read(buf.initialize_unfilled())?;
                buf.advance(bytes_read);
                Poll::Ready(Ok(()))
            }
            None => Poll::Ready(Ok(())),
        })
    }
}
