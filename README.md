# wallbox

Wallbox load management. Charge your car with the surplus energy of
your PV system. Read out and log the parameters of your energy meter.
Monitor your residual current monitor system. I'm developing this for
my own purposes. The name is a bit of a misnomer, as only one
component is concerned with the wallbox.

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
(physically, it doesn't mean that it runs free software).
Specifically:

* An E3DC PV system, S10 X Compact
* A Mennekes Amtron Charge Control electric vehicle charger (the 11kW flavour)
* A Siemens PAC2200 energy meter with integrated transducers
* A Doepke DCTR B-X Hz residual current monitoring system


