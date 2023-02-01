# wallbox
Wallbox load management

I'm developing this for my own purposes, to charge an electric vehicle
with surplus power from a solar power plant. This is work in progress.
It is not yet configurable.

The algorithm will change over time. Right now it is the following:

* Read from the PV system the PV power and the house power, then
  compute the surplus power.
* If the surplus power is greater than 900 Watts, increase the vehicle
  loading current by 1 Ampere.
* Conversely, if the surplus power is negative, decrease the vehicle
  loading current by as many amps as is required to not draw any power
  from the power grid.
