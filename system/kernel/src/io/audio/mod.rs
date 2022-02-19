//! Audio API

use crate::{
    sync::Mutex,
    task::scheduler::{Priority, SpawnOption, Timer},
};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    slice,
    sync::{Arc, Weak},
    vec::Vec,
};
use core::{
    cell::UnsafeCell,
    mem::transmute,
    mem::MaybeUninit,
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

pub type FreqType = f64;
pub type SampleType = f64;

pub const FREQ_MIN: FreqType = 20.0;
pub const FREQ_MAX: FreqType = 20_000.0;
pub const AUDIO_LEVEL_MAX: SampleType = 1.0;
pub const AUDIO_LEVEL_MIN: SampleType = -1.0;

static mut AUDIO_MANAGER: MaybeUninit<UnsafeCell<AudioManager>> = MaybeUninit::uninit();

pub struct AudioManager {
    audio_driver: Mutex<Option<Arc<dyn AudioDriver>>>,
    emitters: Mutex<BTreeMap<AudioHandle, AudioEmitter>>,
}

impl AudioManager {
    pub const DEFAULT_SAMPLE_RATE: FreqType = 44_100.0;

    #[inline]
    pub unsafe fn init() {
        AUDIO_MANAGER = MaybeUninit::new(UnsafeCell::new(AudioManager::new()));

        SpawnOption::with_priority(Priority::High).start_process(
            Self::_audio_thread,
            0,
            "Audio Manager",
        );
    }

    #[inline]
    fn new() -> Self {
        Self {
            audio_driver: Mutex::new(None),
            emitters: Mutex::new(BTreeMap::new()),
        }
    }

    #[inline]
    fn shared<'a>() -> &'a Self {
        unsafe { &*AUDIO_MANAGER.assume_init_ref().get() }
    }

    #[inline]
    fn next_handle() -> AudioHandle {
        static NEXT_HANDLE: AtomicUsize = AtomicUsize::new(1);
        unsafe {
            AudioHandle(NonZeroUsize::new_unchecked(
                NEXT_HANDLE.fetch_add(1, Ordering::SeqCst),
            ))
        }
    }

    #[inline]
    pub unsafe fn set_audio_driver(destination: Arc<dyn AudioDriver>) {
        *Self::shared().audio_driver.lock().unwrap() = Some(destination);
    }

    pub fn master_gain() -> SampleType {
        0.01
    }

    #[inline]
    pub fn reinterpret_i16(src: SampleType) -> i16 {
        (src * i16::MAX as SampleType) as i16
    }

    #[inline]
    pub fn schedule_emitter(emitter: AudioEmitter) -> AudioHandle {
        let shared = Self::shared();
        let handle = emitter.handle;

        shared.emitters.lock().unwrap().insert(handle, emitter);

        handle
    }

    #[inline]
    pub fn remove_emitter(handle: AudioHandle) {
        let shared = Self::shared();
        let _ = shared.emitters.lock().unwrap().remove(&handle);
    }

    fn _audio_thread(_: usize) {
        let shared = Self::shared();

        let driver = loop {
            Timer::sleep(Duration::from_millis(100));
            match Self::shared().audio_driver.lock().unwrap().clone() {
                Some(v) => break v,
                None => (),
            }
        };

        let buffer_len = driver.size_of_buffer();
        let mut buffer = Vec::with_capacity(buffer_len);
        buffer.resize(buffer_len, 0);

        let mut buffer_mute = Vec::with_capacity(buffer_len);
        buffer_mute.resize(buffer_len, 0);

        let wave_len = buffer_len / 4;
        let wave_buffer =
            unsafe { slice::from_raw_parts_mut(transmute(buffer.get_unchecked_mut(0)), wave_len) };

        loop {
            let mut emitters = shared.emitters.lock().unwrap();
            let is_mute = if emitters.len() > 0 {
                let master_gain = Self::master_gain();
                for data in wave_buffer.iter_mut() {
                    let mut sum = 0.0;
                    for emitter in emitters.values_mut() {
                        let mut data = master_gain;
                        for filter in emitter.filters.iter_mut() {
                            data = (filter)(data);
                        }
                        sum += data;
                    }
                    *data = (Self::reinterpret_i16(sum) as u32) * 0x0001_0001;
                }
                false
            } else {
                true
            };
            drop(emitters);
            if is_mute {
                loop {
                    if driver.write_block(buffer_mute.as_slice()).is_some() {
                        break;
                    }
                    Timer::sleep(Duration::from_millis(10));
                }
            } else {
                loop {
                    if driver.write_block(buffer.as_slice()).is_some() {
                        break;
                    }
                    Timer::sleep(Duration::from_millis(10));
                }
            }
        }
    }
}

pub trait AudioDriver {
    fn size_of_buffer(&self) -> usize;

    fn write_block(&self, data: &[u8]) -> Option<()>;
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AudioHandle(NonZeroUsize);

impl AudioHandle {
    pub fn stop(&self) {
        AudioManager::remove_emitter(*self);
    }
}

pub struct AudioEmitter {
    handle: AudioHandle,
    filters: Vec<Box<dyn AudioNodeFilter>>,
}

impl AudioEmitter {
    #[inline]
    pub fn new(filters: Vec<Box<dyn AudioNodeFilter>>) -> Self {
        Self {
            handle: AudioManager::next_handle(),
            filters,
        }
    }

    #[inline]
    pub const fn handle(&self) -> AudioHandle {
        self.handle
    }
}

pub struct AudioContext {
    handles: Mutex<Vec<AudioHandle>>,
}

impl AudioContext {
    #[inline]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            handles: Mutex::new(Vec::new()),
        })
    }

    #[inline]
    pub fn create_oscillator(
        self: &Arc<Self>,
        freq: FreqType,
        osc_type: OscType,
    ) -> Box<AudioNode> {
        let ctx = Arc::downgrade(self);
        let length = AudioManager::DEFAULT_SAMPLE_RATE / freq;
        match osc_type {
            OscType::Square => SquareWaveOscillator::new(ctx, length),
            OscType::Sine => todo!(),
            OscType::Sawtooth => SawtoothWaveOscillator::new(ctx, length),
            OscType::Triangle => TriangleWaveOscillator::new(ctx, length),
        }
    }

    #[inline]
    pub fn create_gain(self: &Arc<Self>, gain: SampleType) -> Box<AudioNode> {
        let ctx = Arc::downgrade(self);
        AudioNode::new(ctx, move |v| v * gain)
    }

    #[inline]
    pub fn destination(self: &Arc<Self>) -> Box<AudioNode> {
        AudioNode::closed(Arc::downgrade(self))
    }

    pub fn schedule(self: &Arc<Self>, emitter: AudioEmitter) -> AudioHandle {
        let handle = emitter.handle();
        self.handles.lock().unwrap().push(handle);
        AudioManager::schedule_emitter(emitter)
    }
}

impl Drop for AudioContext {
    fn drop(&mut self) {
        let vec = self.handles.lock().unwrap();
        for handle in vec.iter() {
            handle.stop();
        }
    }
}

/// Oscillator Type
#[derive(Debug, Clone, Copy)]
pub enum OscType {
    Square,
    Sine,
    Sawtooth,
    Triangle,
}

pub trait AudioNodeFilter = FnMut(SampleType) -> SampleType + 'static;

pub struct AudioNode {
    ctx: Weak<AudioContext>,
    filter: Box<dyn AudioNodeFilter>,
    destination: Option<Box<AudioNode>>,
    is_closed: bool,
}

impl AudioNode {
    #[inline]
    pub fn new<F>(ctx: Weak<AudioContext>, filter: F) -> Box<Self>
    where
        F: AudioNodeFilter,
    {
        Box::new(Self {
            ctx,
            filter: Box::new(filter),
            destination: None,
            is_closed: false,
        })
    }

    #[inline]
    pub fn closed(ctx: Weak<AudioContext>) -> Box<Self> {
        Box::new(Self {
            ctx,
            filter: Box::new(|_| SampleType::NAN),
            destination: None,
            is_closed: true,
        })
    }

    #[inline]
    pub const fn number_of_outputs(&self) -> usize {
        self.destination.is_some() as usize
    }

    #[inline]
    pub const fn is_leaf(&self) -> bool {
        self.number_of_outputs() == 0
    }

    #[inline]
    pub const fn is_closed(&self) -> bool {
        self.is_closed
    }

    #[inline]
    pub fn connect(&mut self, destination: Box<AudioNode>) {
        if !self.is_closed() {
            self.destination = Some(destination);
        }
    }

    #[inline]
    pub fn into_filter(self) -> Box<dyn AudioNodeFilter> {
        self.filter
    }

    pub fn final_destination(&self) -> Option<&Box<AudioNode>> {
        if let Some(destination) = self.destination.as_ref() {
            if destination.is_leaf() {
                self.destination.as_ref()
            } else {
                destination.final_destination()
            }
        } else {
            None
        }
    }

    pub fn take_final_destination(&mut self) -> Option<Box<AudioNode>> {
        if let Some(destination) = self.destination.as_mut() {
            if destination.is_leaf() {
                self.destination.take()
            } else {
                destination.take_final_destination()
            }
        } else {
            None
        }
    }

    pub fn start(mut self) -> Result<AudioHandle, Self> {
        // The final destination must exist and be closed
        if let Some(leaf) = self.final_destination() {
            if !leaf.is_closed() {
                return Err(self);
            }
        } else if !self.is_closed() {
            return Err(self);
        }
        let ctx = match self.ctx.upgrade() {
            Some(v) => v,
            None => return Err(self),
        };

        let _ = self.take_final_destination();
        let mut vec = Vec::new();
        while let Some(leaf) = self.take_final_destination() {
            vec.push(leaf);
        }
        vec.push(Box::new(self));
        let filters: Vec<Box<dyn AudioNodeFilter>> =
            vec.into_iter().rev().map(|v| v.into_filter()).collect();

        Ok(ctx.schedule(AudioEmitter::new(filters)))
    }
}

pub struct SquareWaveOscillator {
    length: f64,
    sign: f64,
    time: f64,
}

impl SquareWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64) -> Box<AudioNode> {
        let mut this = Self {
            length: length / 2.0,
            sign: 1.0,
            time: 0.0,
        };
        AudioNode::new(ctx, move |data| {
            this.time = this.time + 1.0;
            if this.time > this.length {
                this.time -= this.length;
                this.sign = -this.sign;
            }
            data * this.sign
        })
    }
}

/// TODO:
pub struct SawtoothWaveOscillator {
    length: f64,
    time: f64,
}

impl SawtoothWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64) -> Box<AudioNode> {
        let mut this = Self { length, time: 0.0 };
        AudioNode::new(ctx, move |data| {
            this.time = this.time + 1.0;
            if this.time > this.length {
                this.time -= this.length;
            }
            data * this.time / this.length
        })
    }
}

/// TODO:
pub struct TriangleWaveOscillator {
    length: f64,
    sign: f64,
    time: f64,
}

impl TriangleWaveOscillator {
    pub fn new(ctx: Weak<AudioContext>, length: f64) -> Box<AudioNode> {
        let mut this = Self {
            length: length / 2.0,
            sign: 1.0,
            time: 0.0,
        };
        AudioNode::new(ctx, move |data| {
            this.time = this.time + 1.0;
            if this.time > this.length {
                this.time -= this.length;
                this.sign = -this.sign;
            }
            if this.sign > 0.0 {
                data * this.time / this.length
            } else {
                data * this.time / this.length * -1.0
            }
        })
    }
}