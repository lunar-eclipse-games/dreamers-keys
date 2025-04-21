use std::str::FromStr;

use instance_server::run;
use uuid::Uuid;

fn main() {
    let mut args = std::env::args();

    args.next().unwrap();

    let id = Uuid::from_str(&args.next().unwrap()).unwrap();
    let key: [u8; 32] = hex::decode(args.next().unwrap()).unwrap().try_into().unwrap();

    run(id, key);
}
