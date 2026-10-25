//! Optional GPU telemetry from the installed vendor driver.
//!
//! Nothing here is required: `nvml.dll` and `atiadlxx.dll` are loaded lazily,
//! every symbol is optional, and any failure returns `None`. No vendor SDK is
//! linked and nothing is redistributed. The provider is read on the GPU's slow
//! refresh cadence (3 s), so the cost stays in the noise.
//!
//! * NVIDIA: NVML, which reports temperature, fan, power, clocks and load.
//! * AMD: ADL. The PMLog query (edge/hotspot temperature, fan, ASIC/APU power,
//!   clocks, activity) is preferred; Overdrive5 is the fallback. The PMLog entry
//!   point is deprecated and absent on some drivers, hence both paths.
//!
//! The allocation callback ADL requires is deliberately a one-shot allocation:
//! ADL owns it internally and the SDK exposes no matching free callback. It is
//! bounded by the adapter-enumeration buffers, not by the sampling path.

use std::ffi::c_void;
use std::mem::{size_of, transmute_copy};
use std::ptr;
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{PCSTR, PCWSTR};

/// Values a vendor driver reported this cycle. All fields are best effort.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VendorReading {
    pub name: Option<String>,
    pub temperature_c: Option<f32>,
    pub fan_percent: Option<f32>,
    pub power_w: Option<f32>,
    pub core_mhz: Option<u32>,
    pub memory_mhz: Option<u32>,
    pub utilization_percent: Option<f32>,
}

/// A lazily opened vendor backend. Dropping it shuts the vendor API down.
pub struct VendorSensors {
    backend: Backend,
}

enum Backend {
    Nvml(Nvml),
    Adl(Adl),
}

impl VendorSensors {
    /// Try NVIDIA first (NVML enumerates only NVIDIA devices), then AMD.
    pub fn open() -> Option<Self> {
        if let Some(nvml) = Nvml::open() {
            return Some(Self {
                backend: Backend::Nvml(nvml),
            });
        }
        Adl::open().map(|adl| Self {
            backend: Backend::Adl(adl),
        })
    }

    /// Read the current values. `None` means the driver had nothing to say.
    pub fn read(&mut self) -> Option<VendorReading> {
        match &mut self.backend {
            Backend::Nvml(nvml) => nvml.read(),
            Backend::Adl(adl) => adl.read(),
        }
    }
}

/// A DLL that unloads itself.
struct Library(HMODULE);

impl Library {
    fn load(name: &str) -> Option<Self> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is a null-terminated UTF-16 string that outlives the call.
        let module = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr())) }.ok()?;
        Some(Self(module))
    }

    /// Resolve a symbol into a typed function pointer.
    ///
    /// # Safety
    ///
    /// `T` must be the exact C signature the vendor exports for `name`.
    unsafe fn symbol<T: Copy>(&self, name: &'static [u8]) -> Option<T> {
        debug_assert_eq!(size_of::<T>(), size_of::<usize>());
        let address = unsafe { GetProcAddress(self.0, PCSTR(name.as_ptr())) }?;
        // SAFETY: the caller guarantees the signature; both are code pointers
        // of pointer width.
        Some(unsafe { transmute_copy::<unsafe extern "system" fn() -> isize, T>(&address) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: the module was loaded here and is not used after this point.
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

fn trim_nul(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

// ---------------------------------------------------------------------------
// NVIDIA NVML
// ---------------------------------------------------------------------------

const NVML_SUCCESS: i32 = 0;
const NVML_TEMPERATURE_GPU: i32 = 0;
const NVML_CLOCK_GRAPHICS: i32 = 0;
const NVML_CLOCK_MEM: i32 = 2;

type NvmlInit = unsafe extern "C" fn() -> i32;
type NvmlShutdown = unsafe extern "C" fn() -> i32;
type NvmlCount = unsafe extern "C" fn(*mut u32) -> i32;
type NvmlHandle = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
type NvmlName = unsafe extern "C" fn(*mut c_void, *mut i8, u32) -> i32;
type NvmlTemperature = unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32;
type NvmlFan = unsafe extern "C" fn(*mut c_void, *mut u32) -> i32;
type NvmlPower = unsafe extern "C" fn(*mut c_void, *mut u32) -> i32;
type NvmlClock = unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32;
type NvmlUtilization = unsafe extern "C" fn(*mut c_void, *mut NvmlUtilizationRates) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct NvmlUtilizationRates {
    gpu: u32,
    memory: u32,
}

struct Nvml {
    _library: Library,
    device: *mut c_void,
    shutdown: NvmlShutdown,
    name: Option<String>,
    temperature: Option<NvmlTemperature>,
    fan: Option<NvmlFan>,
    power: Option<NvmlPower>,
    clock: Option<NvmlClock>,
    utilization: Option<NvmlUtilization>,
}

impl Nvml {
    fn open() -> Option<Self> {
        let library = Library::load("nvml.dll")
            .or_else(|| Library::load(r"C:\Program Files\NVIDIA Corporation\NVSMI\nvml.dll"))?;

        // SAFETY: every signature below matches the NVML C API.
        let init: NvmlInit = unsafe { library.symbol(b"nvmlInit_v2\0") }
            .or_else(|| unsafe { library.symbol(b"nvmlInit\0") })?;
        // SAFETY: see above.
        let shutdown: NvmlShutdown = unsafe { library.symbol(b"nvmlShutdown\0") }?;
        // SAFETY: see above.
        let count: NvmlCount = unsafe { library.symbol(b"nvmlDeviceGetCount_v2\0") }
            .or_else(|| unsafe { library.symbol(b"nvmlDeviceGetCount\0") })?;
        // SAFETY: see above.
        let handle: NvmlHandle = unsafe { library.symbol(b"nvmlDeviceGetHandleByIndex_v2\0") }
            .or_else(|| unsafe { library.symbol(b"nvmlDeviceGetHandleByIndex\0") })?;

        // SAFETY: no arguments; NVML is initialized once for this handle.
        if unsafe { init() } != NVML_SUCCESS {
            return None;
        }

        let mut devices = 0u32;
        // SAFETY: `devices` is a valid out pointer.
        if unsafe { count(&mut devices) } != NVML_SUCCESS || devices == 0 {
            // SAFETY: balances the successful init above.
            unsafe {
                let _ = shutdown();
            }
            return None;
        }

        let mut device = ptr::null_mut();
        // SAFETY: `device` is a valid out pointer; index 0 exists.
        if unsafe { handle(0, &mut device) } != NVML_SUCCESS || device.is_null() {
            // SAFETY: balances the successful init above.
            unsafe {
                let _ = shutdown();
            }
            return None;
        }

        // SAFETY: all symbols below are optional NVML exports with the signatures
        // declared above; missing symbols stay `None`.
        let temperature: Option<NvmlTemperature> =
            unsafe { library.symbol(b"nvmlDeviceGetTemperature\0") };
        // SAFETY: see above.
        let fan: Option<NvmlFan> = unsafe { library.symbol(b"nvmlDeviceGetFanSpeed\0") };
        // SAFETY: see above.
        let power: Option<NvmlPower> = unsafe { library.symbol(b"nvmlDeviceGetPowerUsage\0") };
        // SAFETY: see above.
        let clock: Option<NvmlClock> = unsafe { library.symbol(b"nvmlDeviceGetClockInfo\0") };
        // SAFETY: see above.
        let utilization: Option<NvmlUtilization> =
            unsafe { library.symbol(b"nvmlDeviceGetUtilizationRates\0") };
        // SAFETY: see above.
        let name_fn: Option<NvmlName> = unsafe { library.symbol(b"nvmlDeviceGetName\0") };

        let name = name_fn.and_then(|function| {
            let mut buffer = [0i8; 96];
            // SAFETY: `buffer` is a valid writable array and the length is exact.
            let status = unsafe { function(device, buffer.as_mut_ptr(), buffer.len() as u32) };
            (status == NVML_SUCCESS).then(|| {
                let bytes: Vec<u8> = buffer
                    .iter()
                    .map(|value| *value as u8)
                    .take_while(|value| *value != 0)
                    .collect();
                String::from_utf8_lossy(&bytes).trim().to_string()
            })
        });
        let name = name.filter(|name| !name.is_empty());

        Some(Self {
            _library: library,
            device,
            shutdown,
            name,
            temperature,
            fan,
            power,
            clock,
            utilization,
        })
    }

    fn read(&mut self) -> Option<VendorReading> {
        let mut reading = VendorReading {
            name: self.name.clone(),
            ..VendorReading::default()
        };

        if let Some(function) = self.temperature {
            let mut value = 0u32;
            // SAFETY: the handle is live and `value` is a valid out pointer.
            if unsafe { function(self.device, NVML_TEMPERATURE_GPU, &mut value) } == NVML_SUCCESS
                && (1..=150).contains(&value)
            {
                reading.temperature_c = Some(value as f32);
            }
        }

        if let Some(function) = self.fan {
            let mut value = 0u32;
            // SAFETY: as above.
            if unsafe { function(self.device, &mut value) } == NVML_SUCCESS && value <= 100 {
                reading.fan_percent = Some(value as f32);
            }
        }

        if let Some(function) = self.power {
            let mut value = 0u32;
            // SAFETY: as above.
            if unsafe { function(self.device, &mut value) } == NVML_SUCCESS
                && (1..=2_000_000).contains(&value)
            {
                reading.power_w = Some(value as f32 / 1000.0);
            }
        }

        if let Some(function) = self.clock {
            for (clock_type, slot) in [
                (NVML_CLOCK_GRAPHICS, &mut reading.core_mhz),
                (NVML_CLOCK_MEM, &mut reading.memory_mhz),
            ] {
                let mut value = 0u32;
                // SAFETY: the handle is live and `value` is a valid out pointer.
                if unsafe { function(self.device, clock_type, &mut value) } == NVML_SUCCESS
                    && (1..=10_000).contains(&value)
                {
                    *slot = Some(value);
                }
            }
        }

        if let Some(function) = self.utilization {
            let mut values = NvmlUtilizationRates::default();
            // SAFETY: `values` is a valid out pointer for the documented struct.
            if unsafe { function(self.device, &mut values) } == NVML_SUCCESS && values.gpu <= 100 {
                reading.utilization_percent = Some(values.gpu as f32);
            }
        }

        (reading.name.is_some()
            || reading.temperature_c.is_some()
            || reading.fan_percent.is_some()
            || reading.power_w.is_some()
            || reading.core_mhz.is_some()
            || reading.memory_mhz.is_some()
            || reading.utilization_percent.is_some())
        .then_some(reading)
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        // SAFETY: `shutdown` balances the successful init in `open`; the
        // library is unloaded after this field is dropped.
        unsafe {
            let _ = (self.shutdown)();
        }
    }
}

// ---------------------------------------------------------------------------
// AMD ADL
// ---------------------------------------------------------------------------

const ADL_MAX_PATH: usize = 256;
const ADL_PMLOG_MAX_SENSORS: usize = 256;
const ADL_OK: i32 = 0;

// PMLog sensor ids from `adl_defines.h` (`ADL_PMLOG_SENSORS`).
const PMLOG_GFX_CLOCK: usize = 1;
const PMLOG_MEM_CLOCK: usize = 2;
const PMLOG_TEMP_EDGE: usize = 8;
const PMLOG_FAN_RPM: usize = 14;
const PMLOG_FAN_PERCENT: usize = 15;
const PMLOG_ACTIVITY_GFX: usize = 19;
const PMLOG_ASIC_POWER: usize = 23;
const PMLOG_TEMP_HOTSPOT: usize = 27;
const PMLOG_TEMP_GFX: usize = 28;
const PMLOG_TEMP_SOC: usize = 29;
const PMLOG_APU_POWER: usize = 46;
const PMLOG_BOARD_POWER: usize = 73;

type AdlMalloc = unsafe extern "system" fn(i32) -> *mut c_void;
type AdlCreate = unsafe extern "system" fn(Option<AdlMalloc>, i32, *mut *mut c_void) -> i32;
type AdlDestroy = unsafe extern "system" fn(*mut c_void) -> i32;
type AdlAdapterCount = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
type AdlAdapterInfoGet = unsafe extern "system" fn(*mut c_void, *mut AdlAdapterInfo, i32) -> i32;
type AdlQueryPmLog = unsafe extern "system" fn(*mut c_void, i32, *mut AdlPmLogDataOutput) -> i32;
type AdlTemperatureGet =
    unsafe extern "system" fn(*mut c_void, i32, i32, *mut AdlTemperature) -> i32;
type AdlFanSpeedGet =
    unsafe extern "system" fn(*mut c_void, i32, i32, *mut AdlFanSpeedValue) -> i32;
type AdlActivityGet = unsafe extern "system" fn(*mut c_void, i32, *mut AdlPmActivity) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct AdlAdapterInfo {
    i_size: i32,
    i_adapter_index: i32,
    str_udid: [u8; ADL_MAX_PATH],
    i_bus_number: i32,
    i_device_number: i32,
    i_function_number: i32,
    i_vendor_id: i32,
    str_adapter_name: [u8; ADL_MAX_PATH],
    str_display_name: [u8; ADL_MAX_PATH],
    i_present: i32,
    i_exist: i32,
    str_driver_path: [u8; ADL_MAX_PATH],
    str_driver_path_ext: [u8; ADL_MAX_PATH],
    str_pnp_string: [u8; ADL_MAX_PATH],
    i_os_display_index: i32,
}

impl AdlAdapterInfo {
    fn zeroed() -> Self {
        Self {
            i_size: size_of::<Self>() as i32,
            i_adapter_index: 0,
            str_udid: [0; ADL_MAX_PATH],
            i_bus_number: 0,
            i_device_number: 0,
            i_function_number: 0,
            i_vendor_id: 0,
            str_adapter_name: [0; ADL_MAX_PATH],
            str_display_name: [0; ADL_MAX_PATH],
            i_present: 0,
            i_exist: 0,
            str_driver_path: [0; ADL_MAX_PATH],
            str_driver_path_ext: [0; ADL_MAX_PATH],
            str_pnp_string: [0; ADL_MAX_PATH],
            i_os_display_index: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlSensorData {
    supported: i32,
    value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AdlPmLogDataOutput {
    size: i32,
    sensors: [AdlSensorData; ADL_PMLOG_MAX_SENSORS],
}

impl AdlPmLogDataOutput {
    fn zeroed() -> Self {
        Self {
            size: size_of::<Self>() as i32,
            sensors: [AdlSensorData::default(); ADL_PMLOG_MAX_SENSORS],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlTemperature {
    i_size: i32,
    i_temperature: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlFanSpeedValue {
    i_size: i32,
    i_speed_type: i32,
    i_fan_speed: i32,
    i_flags: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AdlPmActivity {
    i_size: i32,
    i_engine_clock: i32,
    i_memory_clock: i32,
    i_vddc: i32,
    i_activity_percent: i32,
    i_current_performance_level: i32,
    i_current_bus_speed: i32,
    i_current_bus_lanes: i32,
    i_maximum_bus_lanes: i32,
    i_reserved: i32,
}

struct Adl {
    _library: Library,
    context: *mut c_void,
    adapter_index: i32,
    name: Option<String>,
    destroy: AdlDestroy,
    pmlog: Option<AdlQueryPmLog>,
    temperature: Option<AdlTemperatureGet>,
    fan: Option<AdlFanSpeedGet>,
    activity: Option<AdlActivityGet>,
}

impl Adl {
    fn open() -> Option<Self> {
        let library = Library::load("atiadlxx.dll").or_else(|| Library::load("atiadlxy.dll"))?;

        // SAFETY: signatures follow the ADL SDK.
        let create: AdlCreate = unsafe { library.symbol(b"ADL2_Main_Control_Create\0") }?;
        // SAFETY: see above.
        let destroy: AdlDestroy = unsafe { library.symbol(b"ADL2_Main_Control_Destroy\0") }?;
        // SAFETY: see above.
        let count: AdlAdapterCount =
            unsafe { library.symbol(b"ADL2_Adapter_NumberOfAdapters_Get\0") }?;
        // SAFETY: see above.
        let adapter_info: AdlAdapterInfoGet =
            unsafe { library.symbol(b"ADL2_Adapter_AdapterInfo_Get\0") }?;

        let mut context = ptr::null_mut();
        // SAFETY: the callback is a valid allocator and `context` is a valid
        // out pointer.
        if unsafe { create(Some(adl_malloc), 1, &mut context) } != ADL_OK || context.is_null() {
            return None;
        }

        let mut adapter_count = 0i32;
        // SAFETY: `context` is live and the out pointer is valid.
        if unsafe { count(context, &mut adapter_count) } != ADL_OK || adapter_count <= 0 {
            // SAFETY: balances the successful create above.
            unsafe {
                let _ = destroy(context);
            }
            return None;
        }

        let mut adapters = vec![AdlAdapterInfo::zeroed(); adapter_count as usize];
        let bytes = (adapters.len() * size_of::<AdlAdapterInfo>()) as i32;
        // SAFETY: the vector is `bytes` long and correctly aligned; ADL writes
        // at most that many bytes.
        let status = unsafe { adapter_info(context, adapters.as_mut_ptr(), bytes) };
        if status != ADL_OK {
            // SAFETY: balances the successful create above.
            unsafe {
                let _ = destroy(context);
            }
            return None;
        }

        let adapter = adapters
            .iter()
            .find(|adapter| adapter.i_present != 0 && adapter.i_exist != 0)
            .or_else(|| adapters.first())?;
        let adapter_index = adapter.i_adapter_index;
        let name = trim_nul(&adapter.str_adapter_name);
        let name = (!name.is_empty()).then_some(name);

        // SAFETY: optional exports; absent symbols are fine.
        let pmlog: Option<AdlQueryPmLog> =
            unsafe { library.symbol(b"ADL2_New_QueryPMLogData_Get\0") };
        // SAFETY: see above.
        let temperature: Option<AdlTemperatureGet> =
            unsafe { library.symbol(b"ADL2_Overdrive5_Temperature_Get\0") };
        // SAFETY: see above.
        let fan: Option<AdlFanSpeedGet> =
            unsafe { library.symbol(b"ADL2_Overdrive5_FanSpeed_Get\0") };
        // SAFETY: see above.
        let activity: Option<AdlActivityGet> =
            unsafe { library.symbol(b"ADL2_Overdrive5_CurrentActivity_Get\0") };

        if pmlog.is_none() && temperature.is_none() && fan.is_none() && activity.is_none() {
            // SAFETY: balances the successful create above.
            unsafe {
                let _ = destroy(context);
            }
            return None;
        }

        Some(Self {
            _library: library,
            context,
            adapter_index,
            name,
            destroy,
            pmlog,
            temperature,
            fan,
            activity,
        })
    }

    fn read(&mut self) -> Option<VendorReading> {
        let pmlog = self.pmlog.and_then(|query| {
            let mut output = AdlPmLogDataOutput::zeroed();
            // SAFETY: `context` is live, the index exists, and `output` is sized
            // exactly as the structure requires.
            let status = unsafe { query(self.context, self.adapter_index, &mut output) };
            (status == ADL_OK).then(|| Self::from_pmlog(self.name.clone(), &output))
        });

        let pmlog_has_data = pmlog.as_ref().is_some_and(|reading| {
            reading.temperature_c.is_some()
                || reading.fan_percent.is_some()
                || reading.power_w.is_some()
                || reading.core_mhz.is_some()
                || reading.memory_mhz.is_some()
                || reading.utilization_percent.is_some()
        });
        if pmlog_has_data {
            return pmlog;
        }

        self.read_overdrive5().or(pmlog)
    }

    fn from_pmlog(name: Option<String>, output: &AdlPmLogDataOutput) -> VendorReading {
        let sensor = |id: usize| -> Option<i32> {
            let data = output.sensors.get(id)?;
            (data.supported != 0).then_some(data.value)
        };
        let in_range = |value: Option<i32>, low: i32, high: i32| {
            value.filter(|value| (low..=high).contains(value))
        };

        let temperature = in_range(
            sensor(PMLOG_TEMP_EDGE)
                .or_else(|| sensor(PMLOG_TEMP_GFX))
                .or_else(|| sensor(PMLOG_TEMP_HOTSPOT))
                .or_else(|| sensor(PMLOG_TEMP_SOC)),
            1,
            150,
        )
        .map(|value| value as f32);
        let fan_percent = in_range(sensor(PMLOG_FAN_PERCENT), 0, 100).map(|value| value as f32);
        // Fan RPM is a useful fallback only for the diagnostic; the card shows
        // the percentage, which most laptop firmware reports.
        let _ = sensor(PMLOG_FAN_RPM);
        let power = in_range(sensor(PMLOG_ASIC_POWER), 1, 2000)
            .or_else(|| in_range(sensor(PMLOG_APU_POWER), 1, 2000))
            .or_else(|| in_range(sensor(PMLOG_BOARD_POWER), 1, 2000))
            .map(|value| value as f32);
        let core_mhz = in_range(sensor(PMLOG_GFX_CLOCK), 1, 10_000).map(|value| value as u32);
        let memory_mhz = in_range(sensor(PMLOG_MEM_CLOCK), 1, 10_000).map(|value| value as u32);
        let utilization = in_range(sensor(PMLOG_ACTIVITY_GFX), 0, 100).map(|value| value as f32);

        VendorReading {
            name,
            temperature_c: temperature,
            fan_percent,
            power_w: power,
            core_mhz,
            memory_mhz,
            utilization_percent: utilization,
        }
    }

    fn read_overdrive5(&mut self) -> Option<VendorReading> {
        let mut reading = VendorReading {
            name: self.name.clone(),
            ..VendorReading::default()
        };

        if let Some(function) = self.temperature {
            let mut temperature = AdlTemperature {
                i_size: size_of::<AdlTemperature>() as i32,
                i_temperature: 0,
            };
            // SAFETY: `context` is live and the struct is correctly sized.
            if unsafe { function(self.context, self.adapter_index, 0, &mut temperature) } == ADL_OK
            {
                let celsius = temperature.i_temperature as f32 / 1000.0;
                if (1.0..=150.0).contains(&celsius) {
                    reading.temperature_c = Some(celsius);
                }
            }
        }

        if let Some(function) = self.fan {
            let mut fan = AdlFanSpeedValue {
                i_size: size_of::<AdlFanSpeedValue>() as i32,
                i_speed_type: 1,
                i_fan_speed: 0,
                i_flags: 0,
            };
            // SAFETY: `context` is live and the struct is correctly sized.
            if unsafe { function(self.context, self.adapter_index, 0, &mut fan) } == ADL_OK
                && (0..=100).contains(&fan.i_fan_speed)
            {
                reading.fan_percent = Some(fan.i_fan_speed as f32);
            }
        }

        if let Some(function) = self.activity {
            let mut activity = AdlPmActivity {
                i_size: size_of::<AdlPmActivity>() as i32,
                ..AdlPmActivity::default()
            };
            // SAFETY: `context` is live and the struct is correctly sized.
            if unsafe { function(self.context, self.adapter_index, &mut activity) } == ADL_OK {
                let core = activity.i_engine_clock / 100;
                let memory = activity.i_memory_clock / 100;
                if (1..=10_000).contains(&core) {
                    reading.core_mhz = Some(core as u32);
                }
                if (1..=10_000).contains(&memory) {
                    reading.memory_mhz = Some(memory as u32);
                }
                if (0..=100).contains(&activity.i_activity_percent) {
                    reading.utilization_percent = Some(activity.i_activity_percent as f32);
                }
            }
        }

        (reading.name.is_some()
            || reading.temperature_c.is_some()
            || reading.fan_percent.is_some()
            || reading.core_mhz.is_some()
            || reading.memory_mhz.is_some()
            || reading.utilization_percent.is_some())
        .then_some(reading)
    }
}

impl Drop for Adl {
    fn drop(&mut self) {
        // SAFETY: balances the successful create; the library is unloaded after
        // this field is dropped.
        unsafe {
            let _ = (self.destroy)(self.context);
        }
    }
}

/// ADL's allocation callback. The SDK has no matching free callback, so this is
/// a one-shot allocation during initialization (adapter enumeration), not on the
/// sampling path.
unsafe extern "system" fn adl_malloc(size: i32) -> *mut c_void {
    if size <= 0 {
        return ptr::null_mut();
    }
    let Ok(layout) = std::alloc::Layout::from_size_align(size as usize, 8) else {
        return ptr::null_mut();
    };
    // SAFETY: the layout has non-zero size; the caller must keep the allocation
    // alive, which ADL does for the lifetime of its context.
    unsafe { std::alloc::alloc(layout).cast() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_trimming_stops_at_nul() {
        let mut buffer = [0u8; 8];
        buffer[..4].copy_from_slice(b"AMD ");
        assert_eq!(trim_nul(&buffer), "AMD");
    }

    #[test]
    fn pmlog_prefers_edge_then_gfx_and_falls_back_for_power() {
        let mut output = AdlPmLogDataOutput::zeroed();
        output.sensors[PMLOG_TEMP_EDGE] = AdlSensorData {
            supported: 1,
            value: 61,
        };
        output.sensors[PMLOG_TEMP_GFX] = AdlSensorData {
            supported: 1,
            value: 99,
        };
        output.sensors[PMLOG_FAN_PERCENT] = AdlSensorData {
            supported: 1,
            value: 42,
        };
        output.sensors[PMLOG_ASIC_POWER] = AdlSensorData {
            supported: 1,
            value: 35,
        };
        output.sensors[PMLOG_GFX_CLOCK] = AdlSensorData {
            supported: 1,
            value: 1800,
        };
        output.sensors[PMLOG_MEM_CLOCK] = AdlSensorData {
            supported: 1,
            value: 1200,
        };
        output.sensors[PMLOG_ACTIVITY_GFX] = AdlSensorData {
            supported: 1,
            value: 77,
        };
        let reading = Adl::from_pmlog(Some(String::from("Radeon")), &output);
        assert_eq!(reading.temperature_c, Some(61.0));
        assert_eq!(reading.fan_percent, Some(42.0));
        assert_eq!(reading.power_w, Some(35.0));
        assert_eq!(reading.core_mhz, Some(1800));
        assert_eq!(reading.memory_mhz, Some(1200));
        assert_eq!(reading.utilization_percent, Some(77.0));
        assert_eq!(reading.name.as_deref(), Some("Radeon"));
    }

    #[test]
    fn pmlog_ignores_unsupported_and_implausible_values() {
        let mut output = AdlPmLogDataOutput::zeroed();
        output.sensors[PMLOG_TEMP_EDGE] = AdlSensorData {
            supported: 1,
            value: 300,
        };
        output.sensors[PMLOG_APU_POWER] = AdlSensorData {
            supported: 1,
            value: 18,
        };
        let reading = Adl::from_pmlog(None, &output);
        assert_eq!(reading.temperature_c, None);
        assert_eq!(reading.power_w, Some(18.0));
    }

    #[test]
    fn vendor_open_and_read_never_panic() {
        if let Some(mut sensors) = VendorSensors::open() {
            let _ = sensors.read();
        }
    }
}
