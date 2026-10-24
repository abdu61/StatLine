# Spec — metrics

The sampler runs on its own thread and publishes an immutable `Snapshot`. Everything
here is user-mode and requires no elevation.

## Cadence

| Data | Interval | Notes |
|---|---|---|
| CPU, memory, network, disk I/O | every tick (default 1000 ms) | cheap counters |
| GPU utilization / VRAM | every tick when the GPU module is enabled | PDH wildcard, cached instance list |
| Process table | 2 s, **only while a view that shows processes is visible** | `NtQuerySystemInformation` allocates; ~4.75 ms per query |
| Temperatures (ACPI 5 s, drive 5 min, battery 30 s) | **only while a detail surface is visible** | PDH wildcard and device IOCTLs are not free |
| Static info (CPU model, core counts, volume list) | once, refreshed on `WM_DISPLAYCHANGE` | |

Interval is user-selectable: 500 / 1000 / 2000 / 5000 ms.

## Definitions (must match Task Manager closely; document deviations)

**CPU total %**
`100 * (1 - idle_delta / total_delta)` from `GetSystemTimes`, where
`total_delta = (idle + kernel + user)` deltas. First tick has no predecessor: report
`None`, never a bogus 0.

**CPU per-core %**
`NtQuerySystemInformation(SystemProcessorPerformanceInformation)` →
`SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION[]`; same idle/total math per core.

**CPU MHz**
`CallNtPowerInformation(ProcessorInformation)` → current MHz per core; average for the
strip. Falls back to the registry base clock (`HKLM\HARDWARE\DESCRIPTION\System\
CentralProcessor\0\~MHz`) if unavailable, and the label says "base" in that case.

**Memory**
`GlobalMemoryStatusEx` for total/available; `GetPerformanceInfo` for commit total/limit
and system cache. "Used" = total − available (matches Task Manager's In use).

**Disk**
One cached PDH query opened at startup with `PdhAddEnglishCounterW`
(localization-proof): `\PhysicalDisk(*)\Disk Read Bytes/sec`, `Disk Write Bytes/sec`,
`% Disk Time`. Busy % is clamped to 100. Aggregate across instances for the strip.

**Network**
`GetIfTable2` each tick; sum `InOctets`/`OutOctets` deltas for interfaces that are
`IF_OPER_STATUS_OPERATIONAL` and not loopback/tunnel. Per-adapter rows for the flyout.
Session totals accumulate in the sampler from process start.

**GPU utilization**
PDH `\GPU Engine(*)\Utilization Percentage`, summed per engine and clamped to 100.
Instance list cached for 15 s; on `PDH_INVALID_HANDLE` the query is rebuilt once.
VRAM: `IDXGIAdapter3::QueryVideoMemoryInfo` (`CurrentUsage` / `Budget`).
Absent on systems without GPU performance counters → the module hides.

**GPU temperature / fan / power**
NVML (`nvml.dll`) and AMD ADL (`atiadlxx.dll`) are loaded lazily from the installed
driver. No redistribution, no elevation. Missing DLL or unsupported device → `None`.
**Not implemented yet**: current builds show GPU utilization and VRAM but no vendor
temperature/fan/power; the row hides.

**Battery**
`GetSystemPowerStatus` (percent, AC, seconds left) plus
`CallNtPowerInformation(SystemBatteryState)` (rate) every tick. Health
(`BatteryInformation`) and temperature (`BatteryTemperature`) come from
`IOCTL_BATTERY_QUERY_INFORMATION` on the first battery device interface, cached for
30 s and gated by surface visibility. Firmware that refuses the temperature IOCTL
(`ERROR_INVALID_FUNCTION`) hides the value.

**Thermal (ACPI)**
PDH `\Thermal Zone Information(*)\Temperature`, refreshed every 5 s while a detail
surface is visible. The counter is in **whole degrees Kelvin** (the underlying ACPI
`_TMP` tenths are converted by the PDH bridge), so °C = value − 273.15; values outside
−20..125 °C are discarded rather than displayed. The hottest valid zone wins.
Labelled **System**, never "CPU".

**Disk temperature**
`IOCTL_STORAGE_QUERY_PROPERTY` with `StorageDeviceTemperatureProperty`. The volume is
mapped to a physical disk with `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`. Both IOCTLs cost
~11 ms per volume, so the result is cached for **5 minutes** and only queried while a
detail surface is visible. If a handle is denied or the device does not report a
temperature, the field is `None`.

**Processes**
`NtQuerySystemInformation(SystemProcessInformation)` → PID, image name, user+kernel CPU
time, working set. CPU percent is the CPU-time delta over wall time, expressed as a
share of **total capacity** (the Task Manager scale): a saturated core on an eight-core
machine reads 12.5 %, and the column sums to the CPU total. No delta exists on the first
sample, or for a process that appeared between samples → `None`, which the UI renders
as `—`; never a false `0%`. Images without a name (Idle) are skipped; System (PID 4)
keeps its name.

The query buffer is reused, and the provider is enabled only while the flyout or the
companion is open. Top 12 are kept in the snapshot; the flyout shows 5, the companion 8.
Measured cost: **~4.75 ms** per query on ~250 processes, i.e. ~0.24 % of one core at the
2 s cadence, and zero when every process surface is closed.

**Failure rules**

- A provider that fails logs **once** at warn level, sets its field to `None`, and is
  retried on the next tick. No user-visible error unless a module is enabled and has
  been unavailable for 30 s, in which case the row shows a muted "unavailable" state.
- Counter wraparound and clock gaps (sleep/resume) are handled by treating a negative
  delta as "no sample" for that tick.
- The sampler never allocates per tick beyond the snapshot itself; process/GPU instance
  buffers are reused.

## Out of scope (v1)

CPU package temperature and fan RPM: no user-mode API exists on Windows. GPU vendor
temperatures cover most of the "is it hot" question without shipping a driver.
