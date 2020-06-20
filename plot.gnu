unset key
plot "clobber.log" using 2:3:(stringcolumn(1) eq "Proportional")
plot "clobber.log" using 2:3:(stringcolumn(1) eq "Integral")
plot "clobber.log" using 2:3:(stringcolumn(1) eq "Derivative")
