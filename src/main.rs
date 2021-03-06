use std::env;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::bail;
use serde_json::json;

//use esp_idf_hal::gpio::{Gpio0, Gpio1, Gpio2, Gpio3, Gpio4};
use esp_idf_hal::delay;
use esp_idf_hal::i2c;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::prelude::*;

use embedded_svc::mqtt::client::{Publish, QoS};
use embedded_svc::wifi::*;
use esp_idf_svc::mqtt::client::{EspMqttClient, MqttClientConfiguration};
use esp_idf_svc::netif::EspNetifStack;
use esp_idf_svc::nvs::EspDefaultNvs;
use esp_idf_svc::sysloop::EspSysLoopStack;
use esp_idf_svc::wifi::EspWifi;

use aht20::Aht20;

const SSID: &str = env!("SSID");
const PASS: &str = env!("PASS");
const MQTT_URL: &str = env!("MQTT_URL");
const MQTT_USERNAME: &str = env!("MQTT_USERNAME");
const MQTT_PASSWORD: &str = env!("MQTT_PASSWORD");

fn main() -> anyhow::Result<()> {
    println!("hello");

    // i2c
    let peripherals = Peripherals::take().unwrap();
    let i2c = peripherals.i2c0;
    let sda = peripherals.pins.gpio1;
    let scl = peripherals.pins.gpio2;
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(100.kHz().into());
    let i2c = i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config)?;
    let mut dev = Aht20::new(i2c, delay::FreeRtos).unwrap();

    let mut wifi = Box::new(EspWifi::new(
        Arc::new(EspNetifStack::new()?),
        Arc::new(EspSysLoopStack::new()?),
        Arc::new(EspDefaultNvs::new()?),
    )?);

    println!("Wifi created, about to scan");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        println!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        println!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: SSID.into(),
            password: PASS.into(),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    println!("Wifi configuration set, about to get status");

    wifi.wait_status_with_timeout(Duration::from_secs(20), |status| !status.is_transitional())
        .map_err(|e| anyhow::anyhow!("Unexpected Wifi status: {:?}", e))?;

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(ip_settings))),
        ApStatus::Started(ApIpStatus::Done),
    ) = status
    {
        println!("Wifi connected: {:?}", ip_settings);
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    let mut device = "esp-rs".to_string();
    if let Some(mac) = wifi.with_client_netif(|netif| match netif {
        Some(netif) => netif.get_mac().ok(),
        _ => None,
    }) {
        device = format!("esp-rs_{:02X}{:02X}{:02X}", mac[3], mac[4], mac[5]);
    }
    println!("device: {}", device);
    let topic = format!("tele/{}/SENSOR", device);

    println!("About to start MQTT client");

    let conf = MqttClientConfiguration {
        client_id: Some("aht20"),
        username: Some(MQTT_USERNAME),
        password: Some(MQTT_PASSWORD),
        ..Default::default()
    };

    let mut client = EspMqttClient::new(MQTT_URL, &conf, move |event| {
        println!("MQTT event: {:?}", event);
    })?;

    println!("MQTT client started");

    loop {
        let (h, t) = dev.read().unwrap();
        let (h, t) = (h.rh(), t.celsius());
        let data = json!({"AHT20": {"Temperature": t, "Humidity": h}, "TempUnit": "C"});
        client.publish(&topic, QoS::AtMostOnce, false, data.to_string().as_bytes())?;
        thread::sleep(Duration::from_secs(300));
    }
}
