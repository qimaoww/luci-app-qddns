fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut config = "/etc/config/qddns".to_string();
    let mut once = false;
    let mut idx = 0;

    while idx < args.len() {
        match args[idx].as_str() {
            "--config" => {
                if let Some(value) = args.get(idx + 1) {
                    config = value.clone();
                    idx += 2;
                    continue;
                }
            }
            "--once" => {
                once = true;
                idx += 1;
                continue;
            }
            _ => {}
        }
        idx += 1;
    }

    if let Err(err) = qddns::daemon::run(qddns::daemon::DaemonOptions { config, once }) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
