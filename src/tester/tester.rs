use std::{cmp::max, f64::consts::SQRT_2, fmt::{format, Write}, fs::create_dir_all, sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use metrics_util::Histogram;
use plotters::{backend::BitMapBackend, chart::ChartBuilder, drawing::IntoDrawingArea, prelude::{IntoSegmentedCoord, SegmentValue}, series, style::{text_anchor::{HPos, Pos, VPos}, Color, IntoFont, TextStyle, RED, WHITE}};
use rand::{distributions::Alphanumeric, Rng};
use tokio::{sync::Mutex, time::{self, sleep, Duration}};

use super::client::{TestClient, TestClientHandler};

/// Tester is used to test storage. It uses the client to read / write / delete
/// something from storage.
pub struct Tester<C> where C: TestClient {
    client: Arc<Mutex<C>>,
    random_string: Arc<String>,
}

struct TestResult {
    write_latency: Duration,
    read_latency: Duration,
    delete_latency: Duration,
}

impl<C> Tester<C> where C: TestClient {
    pub fn new(client: C, len: usize) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            random_string: Arc::new(
                rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(len)
                    .map(char::from)
                    .collect()
            ),
        }
    }

    pub async fn test(&mut self) {
        // Init the client.
        let client = self.client.lock().await;
        client.init();
        drop(client);

        // Try write-read-delete ops.
        self.test_try().await;

        // Test.
        self.test_qps(10).await;
        self.test_qps(20).await;
        self.test_qps(50).await;
        self.test_qps(100).await;
        self.test_qps(200).await;
        self.test_qps(500).await;
        self.test_qps(1000).await;
    }

    pub async fn test_try(&mut self) {
        println!("TRY WRITE-READ-DELETE OPS");
        let mut client = self.client.lock().await;
        let key = client.gen_unique_key();
        let hdlr = C::handler();
        hdlr.write(&key, &String::from("Hello World")).await.unwrap();
        let value = hdlr.read(&key).await.unwrap();
        assert!(value == "Hello World");
        hdlr.delete(&key).await.unwrap();
        hdlr.read(&key).await.expect_err("Should return error");
    }

    pub async fn test_qps(&mut self, qps: u64) {
        let mut client = self.client.lock().await;

        // Test.
        let ttime_s = 30;
        println!("TEST:");
        println!("  QPS:           {}", qps);
        println!("  TEST TIME (s): {}", ttime_s);
        let begin_time = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap();
        let mut missed_sleep = 0;
        let mut last_start_time = begin_time;
        let mut handlers = vec![];
        let bar = ProgressBar::new(ttime_s * qps)
            .with_prefix("  BAR: ")
            .with_style(
                ProgressStyle::with_template("{prefix}{wide_bar} {pos}/{len}").unwrap()
            );
        bar.tick();
        for _i in 0..(ttime_s * qps) {
            bar.inc(1);

            // Sleep to make sure the qps is right.
            let this_start_time = last_start_time + Duration::from_micros(1_000_000 / qps);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap();
            if this_start_time > now {
                sleep(this_start_time - now).await;
            } else {
                missed_sleep += 1;
            }
            last_start_time = this_start_time;

            // Query.
            let key = client.gen_unique_key();
            let random_string = self.random_string.clone();
            let handler = tokio::spawn(async move {
                let hdlr = C::handler();

                let write_start = time::Instant::now();
                hdlr.write(&key, &random_string).await.unwrap();
                let write_end = time::Instant::now();

                let read_start = time::Instant::now();
                let value = hdlr.read(&key).await.unwrap();
                let read_end = time::Instant::now();
                assert!(value == *random_string);

                let delete_start = time::Instant::now();
                hdlr.delete(&key).await.unwrap();
                let delete_end = time::Instant::now();

                hdlr.read(&key).await.expect_err("Should return error");

                return TestResult {
                    write_latency: write_end - write_start,
                    read_latency: read_end - read_start,
                    delete_latency: delete_end - delete_start,
                };
            });
            handlers.push(handler);
        }
        bar.finish();

        // Join all.
        let mut write_histogram = create_histogram();
        let mut read_histogram = create_histogram();
        let mut delete_histogram = create_histogram();
        for handler in handlers.into_iter() {
            let result = handler.await.unwrap();
            write_histogram.record(result.write_latency.as_micros() as f64);
            read_histogram.record(result.read_latency.as_micros() as f64);
            delete_histogram.record(result.delete_latency.as_micros() as f64);
        }
        let end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap();
        println!("  DURATION TIME: {:?}", end_time - begin_time);
        println!("  MISSED SLEEP:  {} ({:02}%)", missed_sleep, (missed_sleep as f64) * 100.0 / ((ttime_s * qps) as f64));
        println!("  WRITE HISTOGRAM:");
        show_historgram(&format!("write-qps-{}", qps), &write_histogram);
        println!("  READ HISTOGRAM:");
        show_historgram(&format!("read-qps-{}", qps), &read_histogram);
        println!("  DELETE HISTOGRAM:");
        show_historgram(&format!("delete-qps-{}", qps), &delete_histogram);
    }
}

const BUCKETS: &[f64] = &[
    32., 32. * SQRT_2,
    64., 64. * SQRT_2, 128., 128. * SQRT_2,
    256., 256. * SQRT_2, 512., 512. * SQRT_2,
    1024., 1024. * SQRT_2, 2048., 2048. * SQRT_2,
    4096., 4096. * SQRT_2, 8192., 8192. * SQRT_2,
    16384., 16384. * SQRT_2, 32768., 32768. * SQRT_2,
    65536., 65536. * SQRT_2, 131072., 131072. * SQRT_2,
];
const BUCKETS_LEN: usize = BUCKETS.len();

fn create_histogram() -> Histogram {
    Histogram::new(BUCKETS).unwrap()
}

fn bucket_name(idx: i32) -> String {
    if (idx as usize) >= BUCKETS.len() {
        return "+inf".to_string()
    }
    let time = BUCKETS[idx as usize];
    if time < 1000.0 {
        return format!("{:.2}µs", time);
    } else {
        return format!("{:.2}ms", time / 1000.0);
    }
}

fn show_historgram(name: &str, histogram: &Histogram) {
    let sum = histogram.count();

    // Init the context to draw chart.
    create_dir_all("/tmp/images/").unwrap();
    let picname = format!("/tmp/images/{}.png", name);
    let area = BitMapBackend::new(&picname, ((128 + 64) * 10, 960))
        .into_drawing_area();
    area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&area)
        .margin(64)
        .x_label_area_size(128)
        .y_label_area_size(64 + 32)
        .caption(name, ("sans-serif", 48))
        .build_cartesian_2d((0..(BUCKETS_LEN as i32)).into_segmented(), 0..10000)
        .unwrap();

    // Print in CLI.
    println!("    {}", "-".repeat(10 + 1 + 100 + 1 + 10));
    let mut before = 0;
    for bar in histogram.buckets().clone().into_iter().step_by(2) {
        let dots_num = (((bar.1 - before) * 100 + sum - 1) / sum) as usize;
        let spaces_num = 100 - dots_num;
        println!("    {:10} {}{} {}",
            format!("{:?}", Duration::from_micros(bar.0 as u64)),
            ".".repeat(dots_num),
            " ".repeat(spaces_num),
            bar.1 - before,
        );
        before = bar.1;
    }

    // Build the data to draw the chart
    let mut data = vec![];
    let mut before = 0;
    let mut max_height = 0;
    for bar in histogram.buckets().into_iter().enumerate() {
        let height = ((bar.1.1 - before) * 10000 + sum - 1) / sum;
        data.push((
            bar.0,
            height,
        ));
        max_height = max(max_height, height as u64);
        before = bar.1.1;
    }
    for bar in &mut data {
        bar.1 = bar.1 * 8000 / max_height;
    }

    // Print into the chart.
    chart
        .configure_mesh()
        .disable_x_mesh()
        .y_desc("precent")
        .x_desc("bucket")
        .x_labels(32)
        .x_label_formatter(&|v: &SegmentValue<i32>| {
            match *v {
                SegmentValue::CenterOf(v) => {
                    bucket_name(v)
                }
                _ => panic!("should be CenterOf(i32)"),
            }
        })
        .y_label_formatter(&|v: &i32| {
            format!("{:.2}%", *v * (max_height as i32) / 8000 / 100)
        })
        .y_label_style(("scan-serif", 24))
        .x_label_style(
            TextStyle::from(("scan-serif", 24).into_font())
                .pos(Pos::new(HPos::Left, VPos::Center))
                .transform(plotters::style::FontTransform::Rotate90)
        )
        .axis_desc_style(("sans-serif", 32))
        .draw()
        .unwrap();
    chart.draw_series(
        series::Histogram::vertical(&chart)
            .style(RED.mix(0.5).filled())
            .data(data.iter().map(|d| (d.0 as i32, d.1 as i32))),
    ).unwrap();
    area.present().unwrap();
    println!("    {}", "-".repeat(10 + 1 + 100 + 1 + 10));
    println!("    See also: {}", picname);
    println!("    {}", "-".repeat(10 + 1 + 100 + 1 + 10));
}