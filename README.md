# wallbox

## Important note

The code that adjusts the charging power HAS RECEIVED LITTLE TESTING YET. The
EV charger was installed a few days ago, and I'll probably make adjustments and
fine tune the code over time. USE AT YOUR OWN RISK!!

Update: Today (Feb 22nd 2023) I was able to test the surplus charging algorithm and did identify a few issues which I fixed. More testing is going to happen soon, depending on how much the sun will shine (literally) :-)

## What is it?

EV vehicle charging management. Charge your car with the surplus
energy of your PV system. Read out and log the parameters of your
energy meter. Monitor your residual current monitor system. I'm
developing this for my own purposes. The name is a bit of a misnomer,
as only one component is concerned with the wallbox.

This tool has multiple subcommands. They are:

  * wallbox-manager           Charge your electric car with your PV
                              system's surplus energy
  * energy-meter              Read out, make available and log your
                              energy meter's measurements
  * decompress-stream         Decompress incomplete gzip streams
  * residual-current-monitor  Monitor and logresidual currents and
                              take action when defined thresholds are
                              exceeded (Work in progress!) 

This tool is (currently) fixed to the following hardware that I own
myself (physically, it doesn't mean that it runs free software).
Specifically:

* An E3DC PV system, S10 X Compact
* A Mennekes Amtron Charge Control electric vehicle charger (the 11kW flavour)
* A Siemens PAC2200 energy meter with integrated transducers
* A Doepke DCTR B-X Hz residual current monitoring system


## Getting started

Run ``cargo build --release`` to build the release. (I'm using the musl flavor
to build a static binary that I can deploy to my "solar router" -- an APU2 with
wifi for uplink and some ports to connect the EV charger+PV system to)

Then have a look at ``wallbox.toml``. Edit the file to suit your needs.

Do contact me at hc-solarstrom@hce.li for comments, questions etc. :-)


