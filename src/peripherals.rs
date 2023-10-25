use crate::preludes::*;
use esp_idf_hal::gpio::*;
use esp_idf_hal::i2c::{config::Config as I2cConfig, I2cDriver};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::timer::{TimerConfig, TimerDriver};
use esp_idf_hal::uart::{AsyncUartRxDriver, UartConfig, UartRxDriver};
use esp_idf_hal::units::Hertz;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::timer::EspTaskTimerService;
use esp_idf_svc::wifi::EspWifi;
use esp_idf_sys::{esp_vfs_eventfd_config_t, esp_vfs_eventfd_register};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::sync::Arc;

lazy_static! {
    pub static ref PERIPHERALS: Arc<Mutex<Peripherals>> =
        Arc::new(Mutex::new(Peripherals::take().unwrap()));
    pub static ref SYS_LOOP: EspSystemEventLoop = EspSystemEventLoop::take().unwrap();
    pub static ref NVS_DEFAULT_PARTITION: EspDefaultNvsPartition =
        EspDefaultNvsPartition::take().unwrap();
    pub static ref ESP_TASK_TIMER_SVR: EspTaskTimerService = EspTaskTimerService::new().unwrap();
}

pub fn patch_eventfd() {
    info!("Setting up eventfd...");
    let config = esp_vfs_eventfd_config_t {
        max_fds: 1,
        ..Default::default()
    };
    esp_nofail! { unsafe { esp_vfs_eventfd_register(&config) } }
}

pub fn create_esp_wifi() -> EspWifi<'static> {
    let p = PERIPHERALS.clone();
    let mut p = p.lock();
    let modem = unsafe { p.modem.clone_unchecked() };
    drop(p);
    EspWifi::new(modem, SYS_LOOP.clone(), Some(NVS_DEFAULT_PARTITION.clone())).unwrap()
}

pub fn create_timer_driver_00() -> TimerDriver<'static> {
    let p = PERIPHERALS.clone();
    let mut p = p.lock();
    let timer = unsafe { p.timer00.clone_unchecked() };
    drop(p);
    let config = TimerConfig::new();
    TimerDriver::new(timer, &config).unwrap()
}

macro_rules! define_gpio_input {
    // Arguments are module name and function name of function to test bench
    ($pin_num:expr) => {
        // The macro will expand into the contents of this block.
        paste::item! {
            pub fn [< take_gpio $pin_num _input >]() -> PinDriver<'static, [< Gpio $pin_num >], Input> {
                let p = PERIPHERALS.clone();
                let mut p = p.lock();
                let pin = unsafe { p.pins.[< gpio $pin_num >].clone_unchecked() };
                drop(p);
                PinDriver::input(pin).unwrap()
            }
        }
    };
}

macro_rules! define_gpio_output {
    // Arguments are module name and function name of function to test bench
    ($pin_num:expr) => {
        // The macro will expand into the contents of this block.
        paste::item! {
            pub fn [< take_gpio $pin_num _output >]() -> PinDriver<'static, [< Gpio $pin_num >], Output> {
                let p = PERIPHERALS.clone();
                let mut p = p.lock();
                let pin = unsafe { p.pins.[< gpio $pin_num >].clone_unchecked() };
                drop(p);
                PinDriver::output(pin).unwrap()
            }
        }
    };
}

define_gpio_input!(0);
define_gpio_input!(1);
define_gpio_input!(2);
define_gpio_input!(3);
define_gpio_input!(4);
define_gpio_input!(5);
define_gpio_input!(6);
define_gpio_input!(7);
define_gpio_input!(8);
define_gpio_input!(9); // Button
define_gpio_input!(10);
define_gpio_input!(11);
define_gpio_input!(12);
define_gpio_input!(13);
define_gpio_input!(14);
define_gpio_input!(15);
define_gpio_input!(16);
define_gpio_input!(17);
define_gpio_input!(18);
define_gpio_input!(19);

define_gpio_output!(0);
define_gpio_output!(1);
define_gpio_output!(2);
define_gpio_output!(3);
define_gpio_output!(4);
define_gpio_output!(5);
define_gpio_output!(6);
define_gpio_output!(7);
define_gpio_output!(8);
define_gpio_output!(9);
define_gpio_output!(10);
define_gpio_output!(11);
define_gpio_output!(12); // LED1
define_gpio_output!(13); // LED2
define_gpio_output!(14);
define_gpio_output!(15);
define_gpio_output!(16);
define_gpio_output!(17);
define_gpio_output!(18);
define_gpio_output!(19);

pub fn take_i2c() -> I2cDriver<'static> {
    let p = PERIPHERALS.clone();
    let mut p = p.lock();
    let i2c = unsafe { p.i2c0.clone_unchecked() };
    let pin_sda = unsafe { p.pins.gpio4.clone_unchecked() };
    let pin_scl = unsafe { p.pins.gpio5.clone_unchecked() };
    drop(p);
    let mut config = I2cConfig::default();
    config.sda_pullup_enabled = false;
    config.scl_pullup_enabled = false;
    config.baudrate = Hertz(10_000);
    I2cDriver::new(i2c, pin_sda, pin_scl, &config).unwrap()
}

pub fn take_uart() -> AsyncUartRxDriver<'static, UartRxDriver<'static>> {
    let p = PERIPHERALS.clone();
    let mut p = p.lock();
    let uart = unsafe { p.uart1.clone_unchecked() };
    let rx = unsafe { p.pins.gpio1.clone_unchecked() };

    let conf = UartConfig::new().baudrate(Hertz(4800));
    AsyncUartRxDriver::new(
        uart,
        rx,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        &conf,
    )
    .unwrap()
}
