set datafile separator ","
set title "Title"
set xlabel "Date"
set xdata time
set timefmt "%H:%M:%S"
set format x "%H:%M:%S"
set key left top
plot "examples/.logs/p.log" using 1:2