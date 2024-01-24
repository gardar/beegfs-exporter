use anyhow::{anyhow, Result};
use clap::Parser;
use prometheus_exporter::prometheus::core::{AtomicF64, GenericCounter, GenericGauge};
use prometheus_exporter::prometheus::register_gauge;
use prometheus_exporter::Exporter;
use prometheus_exporter::{self, prometheus::register_counter};
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use libc::{kill, SIGTERM};

#[derive(Parser)]
struct Cli {
    ///Path to the beegfs configuration file
    #[arg(short, long)]
    config_file: Option<PathBuf>,
    ///Port to run on
    #[arg(short, long)]
    bind_address: Option<String>,
    ///Port to run on
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
    ///Max number of crashes before giving up
    #[arg(short, long, default_value_t = 10)]
    restart_attemps: i32,
}

struct BeeGfsExporter {
    exporter: Exporter,
    cli: Cli,
    metric_re: Regex,
    child_pid: Arc<Mutex<Option<u32>>>,
    write_kib: GenericCounter<AtomicF64>,
    read_kib: GenericCounter<AtomicF64>,
    requests: GenericCounter<AtomicF64>,
    queue_len: GenericGauge<AtomicF64>,
    busy: GenericGauge<AtomicF64>,
}

impl BeeGfsExporter {
    fn new() -> BeeGfsExporter {
        let cli = Cli::parse();

        let bind_to = if let Some(bind) = cli.bind_address.clone() {
            bind
        } else {
            "127.0.0.1:13337".to_string()
        };

        let bind_to = bind_to.parse::<SocketAddr>().unwrap();
        let exporter = prometheus_exporter::start(bind_to).unwrap();

        let write_kib =
            register_counter!("beegfs__writen_bytes", "Number of bytes written to BeeGFS").unwrap();
        let read_kib =
            register_counter!("beegfs__read_bytes", "Number of bytes read from BeeGFS").unwrap();
        let requests =
            register_counter!("beegfs__request_total", "Number of requests to BeeGFS").unwrap();
        let queue_len = register_gauge!("beegfs__queue_len", "Length of the BeeGFS queue").unwrap();
        let busy = register_gauge!("beegfs__busy_pct", "BeeGFS load in percent").unwrap();

        let metric_re =
            Regex::new(r"\s+[0-9]+\s+([0-9]+)\s+([0-9]+)\s+([0-9]+)\s+([0-9]+)\s+([0-9]+)")
                .unwrap();

        BeeGfsExporter {
            exporter,
            cli,
            metric_re,
            child_pid: Arc::new(Mutex::new(None)),
            write_kib,
            read_kib,
            requests,
            queue_len,
            busy,
        }
    }

    fn start_monitoring(&self) -> Result<Child> {
        let args: Vec<&str> = vec![
            "beegfs-ctl",
            "--serverstats",
            "--nodetype=storage",
            "--history=1",
            "--logEnabled",
        ];

        let mut args: Vec<String> = args.iter().map(|v| v.to_string()).collect();

        if let Some(cfg) = self.cli.config_file.clone() {
            if !cfg.is_file() {
                return Err(anyhow!("Config file '{}' not found", cfg.to_string_lossy()));
            }

            let target_conf = format!("--cfgFile={}", cfg.to_string_lossy());
            args.push(target_conf);
        }

        let scmd = args.join(" ");
        eprintln!("Running: {}", scmd);

        let child = Command::new(args[0].clone())
            .args(&args[1..])
            .stdout(Stdio::piped())
            .spawn()?;

        Ok(child)
    }

    fn process_events(&self, proc: &mut Child) -> Result<()> {
        if let Some(stdout) = proc.stdout.take() {
            let reader = BufReader::new(stdout);

            for line in reader.lines() {
                match line {
                    Ok(content) => {
                        // Match the regex against the input string
                        if let Some(captures) = self.metric_re.captures(content.as_str()) {
                            // Access captured groups
                            let write = captures[1].parse::<f64>().unwrap();
                            let read = captures[2].parse::<f64>().unwrap();
                            let reqs = captures[3].parse::<f64>().unwrap();
                            let qlen = captures[4].parse::<f64>().unwrap();
                            let bsy = captures[5].parse::<f64>().unwrap();

                            if self.cli.verbose {
                                println!(
                                    "Write {} Read {} Reqs {} Qlen {} Busy {}",
                                    write, read, reqs, qlen, bsy
                                );
                            }

                            self.write_kib.inc_by(write);
                            self.read_kib.inc_by(read);
                            self.requests.inc_by(reqs);
                            self.queue_len.set(qlen);
                            self.busy.set(bsy);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading line: {}", e);
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        let mut error_count = 0;

        let pmut = self.child_pid.clone();

        ctrlc::set_handler(move || {
            /* Get the last pid and kill it */
            if let Ok(v) = pmut.lock() {
                println!("Crtl + C killing subprocess");
                if let Some(pid) = *v {
                    unsafe {
                        kill(pid as i32, SIGTERM);
                    }
                    std::process::exit(1);
                }
            }
        })
        .unwrap();

        /* This should never end */
        loop {
            match self.start_monitoring() {
                Ok(mut child) => {
                    if let Ok(mut v) = self.child_pid.lock() {
                        *v = Some(child.id());
                    }
                    if let Err(e) = self.process_events(&mut child) {
                        eprintln!("beegfs-ctl failed to read output : {}", e);
                    }
                    let _ = child.wait();
                }
                Err(e) => {
                    eprintln!("Failed to run monitoring process : {}", e);
                }
            }

            error_count += 1;

            if error_count > self.cli.restart_attemps {
                return Err(anyhow!(
                    "We saw the command crashing {} times, now giving up",
                    self.cli.restart_attemps
                ));
            }

            sleep(Duration::from_secs(1));
        }
    }
}

fn main() -> Result<()> {
    let mut exporter = BeeGfsExporter::new();

    exporter.run()?;

    Ok(())
}
