use anyhow::Result;
use clap::Parser;
use client::MAX_SEED;
use kitchen::Kitchen;
use rand::Rng;

mod client;
mod kitchen;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

#[derive(Parser)]
struct Args {
    #[arg(long, help = "Challenge server endpoint")]
    pub endpoint: String,

    #[arg(long, help = "Authorization token (required)")]
    pub auth: String,

    #[arg(short, long, default_value_t = String::default(), help = "Problem name (optional)")]
    pub name: String,

    #[arg(
        short,
        long,
        default_value = "0",
        value_parser = clap::value_parser!(u64).range(0..MAX_SEED),
        help = "Problem seed (optional)"
    )]
    pub seed: u64,

    #[arg(
        short,
        long,
        default_value = "500",
        help = "Inverse order rate in milliseconds"
    )]
    rate: u64,

    #[arg(long, default_value = "4", help = "Minimum pickup time in seconds")]
    min: u64,

    #[arg(long, default_value = "8", help = "Maximum pickup time in seconds")]
    max: u64,
}

fn main() -> Result<()> {
    let args = Args::try_parse()?;

    let rate = Duration::from_millis(args.rate);
    let min = Duration::from_secs(args.min);
    let max = Duration::from_secs(args.max);

    // TODO: validate min <= max

    let mut client = client::Client::new(&args.endpoint, &args.auth);
    let (orders, test_id) = client.challenge(&args.name, args.seed)?;

    let kitchen = Arc::new(Kitchen::new());
    let kitchen_clone = kitchen.clone();

    // placements
    let orders_clone = orders.clone();
    let placement_handle = thread::spawn(move || {
        let start_time = SystemTime::now();
        for (idx, order) in orders_clone.iter().enumerate() {
            let placement_time = start_time + rate * idx as u32;

            let now = SystemTime::now();
            if placement_time > now {
                let wait = placement_time.duration_since(now).unwrap();
                thread::sleep(wait);
            }

            kitchen_clone.place_order(order.clone(), SystemTime::now());
        }
    });

    let orders_clone = orders.clone();
    let mut pickup_handles = Vec::new();
    let start_time = SystemTime::now();

    for (idx, order) in orders_clone.iter().enumerate() {
        let kitchen_pickup = kitchen.clone();
        let order_id = order.id.clone();

        let placement_time = start_time + rate * idx as u32;

        let pickup_delay = rand::rng().random_range(min.as_secs()..=max.as_secs());
        let pickup_time = placement_time + Duration::from_secs(pickup_delay);

        let handle = thread::spawn(move || {
            let now = SystemTime::now();
            if pickup_time > now {
                let wait = pickup_time.duration_since(now).unwrap();
                thread::sleep(wait);
            }
            kitchen_pickup.pickup_order(&order_id, SystemTime::now());
        });

        pickup_handles.push(handle);
    }

    placement_handle.join().unwrap();
    for handle in pickup_handles {
        handle.join().unwrap();
    }

    thread::sleep(Duration::from_millis(100)); // give it a bit extra

    let actions = kitchen.get_actions();

    let result = client.solve(&test_id, rate, min, max, &actions)?;

    println!("Test result: {result}");
    Ok(())
}
