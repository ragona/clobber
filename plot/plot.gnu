set datafile separator ","
set title "Now We're Fucking Charting"
set xlabel "Date"
set xdata time
set timefmt "%H:%M:%S"
set format x "%H:%M:%S"
set key off

plot "examples/.logs/p.log" using 1:2  with lines, \
     "examples/.logs/i.log" using 1:2  with lines, \
     "examples/.logs/d.log" using 1:2  with lines, \
     "examples/.logs/pid.log" using 1:2 with lines, \
     "examples/.logs/rps.log" using 1:2  with lines
