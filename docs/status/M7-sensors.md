# M7 — driver-free sensors

**Status:** in progress · drive, battery and ACPI zones are live; GPU vendor sensors pending

## Goal

As much thermal/power data as Windows gives without a kernel driver.

## Checklist

- [x] Drive temperature via `IOCTL_STORAGE_QUERY_PROPERTY`, mapped volume → disk and
      cached for 5 minutes (the device IOCTLs cost ~12 ms)
- [x] Battery health via `IOCTL_BATTERY_QUERY_INFORMATION` (`BatteryInformation`)
- [x] Battery temperature via the same IOCTL where the firmware supports it (this
      machine returns `ERROR_INVALID_FUNCTION`, so it hides)
- [x] ACPI "System" temperature via `\Thermal Zone Information(*)\Temperature`
- [x] Thermal zone refreshed every 5 s, battery details every 30 s, drive temps every
      5 min; failures retry at 1 min and never fake a value
- [x] Flyout/companion show the System card (`°C`, uptime, threads, handles), the Disk
      card appends the drive temperature, and the Battery card appends health and temp
- [ ] GPU temp / fan / power via NVML, loaded lazily from the installed driver
- [ ] AMD path via ADL, same lazy-load contract (this machine is AMD integrated, so
      this needs an AMD discrete GPU to verify)
- [ ] Settings → About explains why CPU package temperature is absent

## Measured on this machine

| Sensor | Cost | Cadence |
|---|---|---|
| ACPI thermal (PDH wildcard) | ~1.13 ms | 5 s |
| Drive temperatures (2 IOCTLs/volume) | ~12 ms | 300 s |
| Battery details (SetupAPI + IOCTL) | ~0.14 ms | 30 s |

The detail providers share the process table's visibility gate, so none of them run
while only the strip is visible. Idle tight-loop cost stays at ~0.46 ms per sample;
opening the flyout or companion raises the average to ~2.3 ms/sample because the
process table dominates (4.75 ms every 2 s).

## Exit criteria

On this machine (and on a machine without GPU counters), no row shows a fabricated
value; unavailable probes cost nothing measurable; no elevation is ever requested.

## Explicitly out of scope

CPU package/per-core temperature and fan RPM. No user-mode API exists. Any future
backend ships as a separate, opt-in component (see the plan's §2.3 and §10.2).
