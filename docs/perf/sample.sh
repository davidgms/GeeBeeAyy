#!/bin/bash
# Samples the phone every 60 s for N minutes while a game runs. Output: temp/perf/session.csv
P="/mnt/d/AndroidSDK/platform-tools/adb.exe -s 7d4c9ae1"
N=${1:-30}
OUT=temp/perf/session.csv
echo "min,frames,janky,cpu_pct,pss_kb,batt_level,batt_temp_c10,thermal,underruns,refresh_hz,game_fps,pid" > $OUT
$P shell dumpsys gfxinfo com.gbee.emulator reset >/dev/null
for m in $(seq 0 $N); do
  pid=$($P shell pidof com.gbee.emulator | tr -d '\r')
  g=$($P shell dumpsys gfxinfo com.gbee.emulator | tr -d "\r")
  fr=$(echo "$g" | grep -m1 "Total frames" | grep -oE '[0-9]+$')
  jk=$(echo "$g" | grep -m1 "Janky frames:" | grep -oE ': [0-9]+' | tr -d ': ')
  cpu=$($P shell top -b -n 1 -d 1 -p $pid 2>/dev/null | awk -v p=$pid '$1==p{print $9}')
  pss=$($P shell dumpsys meminfo $pid | grep -m1 "TOTAL PSS" | grep -oE 'TOTAL PSS: +[0-9]+' | grep -oE '[0-9]+')
  [ -z "$pss" ] && pss=$($P shell dumpsys meminfo $pid | awk '/TOTAL/{print $2; exit}')
  b=$($P shell dumpsys battery | tr -d "\r")
  lvl=$(echo "$b" | grep -m1 " level:" | grep -oE '[0-9]+')
  tmp=$(echo "$b" | grep -m1 "temperature:" | grep -oE '[0-9]+')
  th=$($P shell dumpsys thermalservice | grep -m1 "Thermal Status" | grep -oE '[0-9]+')
  ur=$($P shell dumpsys media.audio_flinger | awk -v p=$pid '$3=="yes" && $4==p {print $(NF-3)}' | head -1)
  hz=$($P shell dumpsys SurfaceFlinger | grep -m1 "refresh-rate" | grep -oE "[0-9.]+")
  # Presented frames of the game's own SurfaceView over its last 128 frames.
  gfps=$($P shell dumpsys SurfaceFlinger --latency "'SurfaceView[com.gbee.emulator/com.gbee.emulator.MainActivity](BLAST)#0'" | tr -d "\r" | awk 'NR>1 && $2>0 && $2<9e18 {n++; if(!f)f=$2; l=$2} END{if(n>1) printf "%.1f", (n-1)/((l-f)/1e9)}')
  echo "$m,$fr,$jk,$cpu,$pss,$lvl,$tmp,$th,$ur,$hz,$gfps,$pid" >> $OUT
  [ $m -lt $N ] && sleep 52
done
echo done
