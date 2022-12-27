use angular_units::Deg;
use async_mutex::Mutex;
use btleplug::{
    api::{
        bleuuid::uuid_from_u16, Central, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        WriteType,
    },
    platform::{Adapter, Manager, Peripheral},
};
use chrono::{DateTime, Datelike, Utc};
use clokwerk::{AsyncScheduler, TimeUnits};
use log::{info, LevelFilter};
use prisma::{FromColor, Hsv, Rgb};
use std::{
    borrow::Borrow, error::Error, sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Arc,
    time::Duration,
};
use tokio::time;
use uuid::Uuid;

const LIGHT_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0x1001);
const CYCLE_TIME_MILLISECOND: u64 = 10;
const MAGIC_NUMBER: u8 = 0x3C;
const CURRENT_LOCATION: (f64, f64) = (47.552922, 19.254477);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    systemd_journal_logger::init().expect("Failed to initialize JournalCTL logger");
    log::set_max_level(LevelFilter::Info);

    let mut scheduler = AsyncScheduler::with_tz(chrono::Utc);

    let light = get_light().await;
    light.connect().await?;
    info!("Connected to lights");
    light.discover_services().await?;
    info!("Discovering light services");

    let light = Arc::new(Mutex::new(light));
    let light_clone = Arc::clone(&light);

    let cmd_char = Arc::new(Mutex::new(
        get_command_characteristics((*light.lock().await).borrow()).await,
    ));
    let cmd_char_clone = Arc::clone(&cmd_char);

    let is_off = Arc::new(AtomicBool::new(false));
    let is_off_clone = Arc::clone(&is_off);
    scheduler.every(2.minutes()).run(move || {
        let is_off_clone = is_off_clone.clone();
        let light_clone = light_clone.clone();
        let cmd_char_clone = cmd_char_clone.clone();
        async move {
            if is_after_sunrise() && is_before_sunset() {
                if !is_off_clone.load(Ordering::Relaxed) {
                    is_off_clone.store(true, Ordering::Relaxed);
                    info!("Turning off lights");
                    turn_off_lights(
                        cmd_char_clone.lock().await.borrow(),
                        (*light_clone.lock().await).borrow(),
                    )
                    .await;
                    info!("Turned off lights");
                }
            } else if is_off_clone.load(Ordering::Relaxed) {
                is_off_clone.store(false, Ordering::Relaxed);
                info!("Turned on lights!");
            }
        }
    });

    let mut hue_deg = 1.0;
    loop {
        scheduler.run_pending().await;

        if !is_off.load(Ordering::Relaxed) {
            hue_deg = (hue_deg + 1.0) % 360.0;
            let hsv = Hsv::new(Deg(hue_deg), 1.0, 1.0);
            let rgb = Rgb::from_color(&hsv);
            let (r, g, b) = rgb_f32_to_u8_capped(rgb);
            set_color(
                (*cmd_char.lock().await).borrow(),
                (*light.lock().await).borrow(),
                (r, g, b),
            )
            .await;

            time::sleep(Duration::from_millis(CYCLE_TIME_MILLISECOND)).await;
        } else {
            time::sleep(Duration::from_secs(60)).await;
        }
    }
}

async fn get_light() -> Peripheral {
    let manager = Manager::new().await.unwrap();
    let central = manager
        .adapters()
        .await
        .expect("Unable to fetch adapter list.")
        .into_iter()
        .next()
        .expect("Unable to find adapters.");
    info!("Found adapter: {:?}", central);

    central.start_scan(ScanFilter::default()).await.ok();
    info!("Starting scan for BLE devices");
    time::sleep(Duration::from_secs(2)).await;

    let light = print_devices(&central)
        .await
        .expect("No Actuel lights found");
    info!("Found lights: {:?}", light);

    light
}

async fn get_command_characteristics(light: &Peripheral) -> Characteristic {
    let chars = light.characteristics();
    let cmd_char = chars
        .iter()
        .find(|c| c.uuid == LIGHT_CHARACTERISTIC_UUID)
        .cloned()
        .expect("Unable to find characterics");
    info!("Found characterics: {}", LIGHT_CHARACTERISTIC_UUID);
    cmd_char
}

async fn print_devices(central: &Adapter) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap() {
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|name| name.contains("Light"))
        {
            return Some(p);
        }
    }
    None
}

async fn set_color(cmd_char: &Characteristic, light: &Peripheral, (r, g, b): (u8, u8, u8)) {
    let color_cmd = vec![MAGIC_NUMBER, 0x02, r, g, b];
    light
        .write(cmd_char, &color_cmd, WriteType::WithoutResponse)
        .await
        .ok();
}

async fn turn_off_lights(cmd_char: &Characteristic, light: &Peripheral) {
    let shut_off_cmd = vec![MAGIC_NUMBER, 0x01];
    light
        .write(cmd_char, &shut_off_cmd, WriteType::WithoutResponse)
        .await
        .ok();
}

fn is_after_sunrise() -> bool {
    let current_date = chrono::Utc::now();
    let (sunrise, _) = get_sunrise_sunset(current_date);

    sunrise < current_date.timestamp()
}

fn is_before_sunset() -> bool {
    let current_date = chrono::Utc::now();
    let (_, sunset) = get_sunrise_sunset(current_date);

    current_date.timestamp() < sunset
}

fn get_sunrise_sunset(current_date: DateTime<Utc>) -> (i64, i64) {
    let (sunrise, sunset) = sunrise::sunrise_sunset(
        CURRENT_LOCATION.0,
        CURRENT_LOCATION.1,
        current_date.year(),
        current_date.month(),
        current_date.day(),
    );

    (sunrise, sunset)
}

fn rgb_f32_to_u8_capped(rgb: Rgb<f32>) -> (u8, u8, u8) {
    (
        (rgb.red() * 255.0) as u8,
        (rgb.green() * 255.0) as u8,
        (rgb.blue() * 255.0) as u8,
    )
}
