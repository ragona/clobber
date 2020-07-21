set datafile separator ","
set title "clobber: pid controlled worker count"
set xlabel "time"
set ylabel "rate per second"
set xdata time
set timefmt "%H:%M:%S"
set format x "%H:%M:%S"
set y2tics 0, 0.001
set ytics nomirror
set terminal wxt size 800, 600
set key left
#set output "plot.png"


plot "examples/.logs/p.log" using 1:2 title "p" axis x1y1 lw 0.5 dt 3, \
     "examples/.logs/i.log" using 1:2 title "i" axis x1y1 lw 0.5 dt 3, \
     "examples/.logs/d.log" using 1:2 title "d" axis x1y1 lw 0.5 dt 3, \
     "examples/.logs/pid.log" using 1:2 title "pid" axis x1y2 lw 0.5 dt 3, \
     "examples/.logs/rps.log" using 1:2 title "rate" axis x1y1 with line lw 3, \
     4000 title "goal"