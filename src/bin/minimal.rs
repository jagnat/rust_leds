#![no_main]
#![no_std]
#![feature(type_alias_impl_trait)]

use rust_leds as _; // global logger + panicking-behavior + memory layout

// TODO(7) Configure the `rtic::app` macro
#[rtic::app(
    // TODO: Replace `some_hal::pac` with the path to the PAC
    device = stm32f4xx_hal::pac,
    // TODO: Replace the `FreeInterrupt1, ...` with free interrupt vectors if software tasks are used
    // You can usually find the names of the interrupt vectors in the some_hal::pac::interrupt enum.
    // dispatchers = [FreeInterrupt1, ...]
)]
mod app {
use rust_leds::util::NUM_LEDS;
use rust_leds::util::TX_BUFFER_SIZE;
use stm32f4xx_hal::dma::MemoryToPeripheral;
// Shared resources go here
    use stm32f4xx_hal::prelude::*;
    use stm32f4xx_hal::gpio::*;
    use stm32f4xx_hal::spi::*;
    use stm32f4xx_hal::dma::*;
    use stm32f4xx_hal::dma::config::DmaConfig;
    use stm32f4xx_hal::pac::*;
    // use embedded_hal::digital::OutputPin;
    use rust_leds::util::*;
    use stm32f4xx_hal::rcc::*;
    use rtic_monotonics::systick::prelude::*;

    systick_monotonic!(Mono, 1000);

    type TxTransfer = Transfer<
        Stream2<DMA2>,
        2,
        Tx<SPI1>,
        MemoryToPeripheral,
        &'static mut [u8; TX_BUFFER_SIZE]>;

    #[shared]
    struct Shared {
        // TODO: Add resources
        tx_transfer: TxTransfer,
    }

    // Local resources go here
    #[local]
    struct Local {
        // TODO: Add resources
        led: PC13<Output>,
        count: i32,
        // spi: Spi<SPI1, true>,
        // tx_buffer: Option<&'static mut [u8; TX_BUFFER_SIZE]>
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        defmt::info!("init");

        let rcc = cx.device.RCC.constrain();
        let mut rcc = rcc.freeze(
            Config::hse(25.MHz())
                .sysclk(96.MHz()),
        );

        Mono::start(cx.core.SYST, rcc.clocks.sysclk().to_Hz());
        let gpioa = cx.device.GPIOA.split(&mut rcc);
        let gpioc = cx.device.GPIOC.split(&mut rcc);

        let sck = gpioa.pa5.into_alternate::<5>().speed(Speed::VeryHigh);
        let mosi = gpioa.pa7.into_alternate::<5>().speed(Speed::VeryHigh);

        let mode = Mode { polarity: Polarity::IdleLow, phase: Phase::CaptureOnFirstTransition, };

        let spi = Spi::new(
            cx.device.SPI1,
            (Some(sck), SPI1::NoMiso, Some(mosi)),
            mode,
            3.MHz(),
            &mut rcc
        );

        let stream = stm32f4xx_hal::dma::StreamsTuple::new(
            cx.device.DMA2,
            &mut rcc
        ).2;

        let tx_buffer = cortex_m::singleton!(: [u8; TX_BUFFER_SIZE] = [0; TX_BUFFER_SIZE]).unwrap();
        fill_transfer_buffer(tx_buffer, &[Rgb::new(255, 0, 0); NUM_LEDS]);

        let tx = spi.use_dma().tx();

        let mut tx_transfer = Transfer::init_memory_to_peripheral(
            stream,
            tx,
            tx_buffer,
            None,
            DmaConfig::default().memory_increment(true).transfer_complete_interrupt(true));

        defmt::info!("pclk2 = {} Hz", rcc.clocks.pclk2().to_Hz());

        let led = gpioc.pc13.into_push_pull_output();

        tx_transfer.start(|_tx| {});

        toggle_led::spawn().ok();
        // diag::spawn().ok();
        // write_leds::spawn().ok();

        let count = 0;

        (
            Shared {
                // Initialization of shared resources go here
                tx_transfer,
            },
            Local {
                led,
                count,
                // spi,
                // Initialization of local resources go here
            },
        )
    }

    // Optional idle, can be removed if not needed.
    // #[idle]
    // fn idle(_: idle::Context) -> ! {
    //     defmt::info!("idle");

    //     loop {
    //         continue;
    //     }
    // }

    #[task(local = [led, count])]
    async fn toggle_led(cx: toggle_led::Context) {
        let led = cx.local.led;

        defmt::debug!("Test");

        loop {
            *cx.local.count += 1;
            led.toggle();
            Mono::delay(500.millis()).await;
        }
    }

    // #[task(local = [spi])]
    // async fn write_leds(cx: write_leds::Context) {
    //     let spi = cx.local.spi;

    //     let mut led_buffer: [Rgb; NUM_LEDS] = [Rgb::new(0, 0, 0); NUM_LEDS];
    //     let mut spi_buffer: [u8; TX_BUFFER_SIZE] = [0; _];

    //     for pix in &mut led_buffer {
    //         *pix = Rgb::new(0, 255, 0);
    //     }

    //     loop {
    //         spi.write(&spi_buffer).unwrap();

    //         Mono::delay(200.millis()).await
    //     }
    // }

    // #[task(shared = [tx_transfer])]
    // async fn diag(mut cx: diag::Context) {
    //     Mono::delay(200.millis()).await;
    //     let ndt = cx.shared.tx_transfer.lock(|t| t.number_of_transfers());
    //     defmt::info!("NDT after 200ms = {}", ndt);
    // }


    #[task(binds=DMA2_STREAM2, shared = [tx_transfer])]
    fn spi_complete(mut cx: spi_complete::Context) {
        cx.shared.tx_transfer.lock(|transfer| {
            transfer.clear_flags(DmaFlag::TransferComplete);
        })
        
    }

    // TODO: Add tasks
    // #[task(priority = 1)]
    // async fn task1(_cx: task1::Context) {
    //     defmt::info!("Hello from task1!");
    // }
}
