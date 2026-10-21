#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: sudo bash tools/usb-button-capture.sh BUS DEVICE [SECONDS]" >&2
  echo "BUS and DEVICE are the decimal numbers printed by lsusb; output is written to stdout." >&2
  exit 2
fi

bus="$1"
device="$2"
seconds="${3:-30}"
if [[ ! "$bus" =~ ^[0-9]+$ || ! "$device" =~ ^[0-9]+$ || ! "$seconds" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
  echo "BUS, DEVICE, and SECONDS must be nonnegative numbers" >&2
  exit 2
fi

monitor="/sys/kernel/debug/usb/usbmon/$((10#$bus))u"
if [[ ! -r "$monitor" ]]; then
  echo "cannot read $monitor" >&2
  echo "load usbmon and mount debugfs first: modprobe usbmon; mount -t debugfs none /sys/kernel/debug" >&2
  exit 1
fi

echo "capturing USB bus $bus device $device for ${seconds}s" >&2
echo "press and release one control at a time; USB packets follow on stdout" >&2
set +e
timeout --signal=INT "${seconds}s" \
  awk -v bus="$((10#$bus))" -v device="$((10#$device))" '
    {
      split($4, address, ":")
      if ((address[2] + 0) == bus && (address[3] + 0) == device) {
        print
        fflush()
      }
    }
  ' "$monitor"
status=$?
set -e
if [[ $status -ne 0 && $status -ne 124 && $status -ne 130 ]]; then
  exit "$status"
fi
