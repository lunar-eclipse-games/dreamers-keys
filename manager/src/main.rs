use manager::run;

fn main() {
    let mut args = std::env::args();

    args.next().unwrap();

    let kind = args.next().unwrap_or("".to_owned());

    let kind = match kind.as_str() {
        "local" => manager::ManagerKind::Local,
        _ => manager::ManagerKind::Online,
    };

    run(kind);
}
