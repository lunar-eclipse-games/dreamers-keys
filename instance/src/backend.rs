use common::Result;
use std::{
    io::{BufRead as _, BufReader, Write as _},
    net::SocketAddr,
};

#[derive(Debug)]
#[non_exhaustive]
pub enum Message {
    Shutdown,
    Teapot,
}

#[derive(Debug)]
pub struct PipeComm {
    tx: interprocess::unnamed_pipe::Sender,
    rx: std::sync::mpsc::Receiver<Message>,
}

#[derive(Debug)]
pub enum BackendCommunication {
    Pipe(PipeComm),
    None,
}

impl BackendCommunication {
    pub fn pipe(
        tx: interprocess::unnamed_pipe::Sender,
        rx: interprocess::unnamed_pipe::Recver,
    ) -> BackendCommunication {
        let (msg_tx, msg_rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let mut rx = BufReader::new(rx);

            let mut msg = String::new();
            loop {
                msg.clear();

                rx.read_line(&mut msg).unwrap();

                let msg = msg.trim();

                if msg == "shutdown" {
                    msg_tx.send(Message::Shutdown).unwrap();
                } else if msg == "teapot" {
                    msg_tx.send(Message::Teapot).unwrap();
                }
            }
        });

        BackendCommunication::Pipe(PipeComm { tx, rx: msg_rx })
    }

    pub fn notify_ready(&mut self, server_addr: SocketAddr) -> Result<()> {
        match self {
            BackendCommunication::Pipe(PipeComm { tx, .. }) => {
                tx.write_all(format!("{server_addr}\n").as_bytes())?;
            }
            BackendCommunication::None => {}
        }

        Ok(())
    }

    pub fn message(&mut self) -> Option<Message> {
        match self {
            BackendCommunication::Pipe(PipeComm { rx, .. }) => rx.try_recv().ok(),
            BackendCommunication::None => None,
        }
    }
}
