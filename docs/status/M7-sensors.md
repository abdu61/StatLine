# M7 — driver-free sensors

**Status:** not started

## Goal

As much thermal/power data as Windows gives without a kernel driver.

## Checklist

- [ ] GPU temp / fan / power via NVML, loaded lazily from the installed driver
- [ ] AMD path via ADL, same lazy-load contract
- [ ] Drive temperature via `IOCTL_STORAGE_QUERY_PROPERTY`, best effort
- [ ] Battery temperature + health via `IOCTL_BATTERY_QUERY_INFORMATION`
- [ ] ACPI "System" temperature via `\Thermal Zone Information(*)\Temperature`
- [ ] Every source degrades to hidden, never to a fake value
- [ ] Settings → About explains why CPU package temperature is absent

## Exit criteria

On this machine (and on a machine without GPU counters), no row shows a fabricated
value; unavailable probes cost nothing measurable; no elevation is ever requested.

## Explicitly out of scope

CPU package/per-core temperature and fan RPM. No user-mode API exists. Any future
backend ships as a separate, opt-in component (see the plan's §2.3 and §10.2).
