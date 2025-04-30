use std::{os::fd::FromRawFd, str::FromStr};

use common::{Error, Result, ResultExt};
use instance::{backend::BackendCommunication, run};
use uuid::Uuid;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut args = std::env::args();

    args.next().unwrap();

    let id = match args.next() {
        Some(id) => Uuid::from_str(&id)?,
        None => Uuid::now_v7(),
    };

    let key: [u8; 32] = match args.next() {
        Some(key) => hex::decode(key)?
            .try_into()
            .map_err(|_| Error::InvalidKeyLength)?,
        None => renet_netcode::generate_random_bytes(),
    };

    let comm = match args.next() {
        Some(comm) => {
            let mut handles = comm.split(';');
            let tx_handle = handles.next().unwrap();
            let tx_handle = tx_handle.parse().context("Invalid Pipe Handle")?;
            let rx_handle = handles.next().unwrap();
            let rx_handle = rx_handle.parse().context("Invalid Pipe Handle")?;

            let tx = unsafe { interprocess::unnamed_pipe::Sender::from_raw_fd(tx_handle) };
            let rx = unsafe { interprocess::unnamed_pipe::Recver::from_raw_fd(rx_handle) };

            BackendCommunication::pipe(tx, rx)
        }
        None => BackendCommunication::None,
    };

    run(id, key, comm)
}
