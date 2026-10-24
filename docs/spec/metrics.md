# Spec — metrics

The sampler runs on its own thread and publishes an immutable `Snapshot`. Everything
here is user-mode and requires no elevation.

## Cadence

| Data | Interval | Notes |
|---|---|---|
| CPU, memory, network, disk I/O | every tick (default 1000 ms) | cheap counters |
| GPU utilization / VRAM | every tick when the GPU module is enabled | PDH wildcard, cached instance list |
| Process table | 5 s, **only while a view that shows processes is visible** | `NtQuerySystemInformation` allocates |
| Temperatures (GPU, disk, battery, ACPI) | 5 s | vendor libs / IOCTLs are not free |
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

**Battery**
`GetSystemPowerStatus` (percent, AC, seconds left) plus
`CallNtPowerInformation(SystemBatteryState)` (rate). Health from
`IOCTL_BATTERY_QUERY_INFORMATION` (`BatteryInformation`: designed vs full capacity)
and temperature where the firmware reports it.

**Thermal (ACPI)**
PDH `\Thermal Zone Information(*)\Temperature` (deci-Kelvin → °C). Labelled **System**,
never "CPU". Frequently unavailable on desktops; that is fine.

**Disk temperature**
`IOCTL_STORAGE_QUERY_PROPERTY` with `StorageDeviceTemperatureProperty`, best effort.
If the handle is denied or the device does not report it, the row disappears.

**Processes**
`NtQuerySystemInformation(SystemProcessInformation)` → PID, image name, working set,
CPU time delta since last sample, thread count. Sorted descending by CPU, top N.

## Failure rules

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
