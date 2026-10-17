#!/bin/bash
# Average CPU% of the app over 10 samples; prints "label avg game_fps underruns"
P="/mnt/d/AndroidSDK/platform-tools/adb.exe -s 7d4c9ae1"
pid=$($P shell pidof com.gbee.emulator | tr -d '\r')
s=0; for i in $(seq 1 10); do c=$($P shell top -b -n 1 -d 1 -p $pid | awk -v p=$pid '$1==p{print $9}'); s=$(echo "$s + $c" | bc); done
g=$($P shell dumpsys SurfaceFlinger --latency "'SurfaceView[com.gbee.emulator/com.gbee.emulator.MainActivity](BLAST)#0'" | tr -d "\r" | awk 'NR>1 && $2>0 && $2<9e18 {n++; if(!f)f=$2; l=$2} END{if(n>1) printf "%.1f", (n-1)/((l-f)/1e9)}')
u=$($P shell dumpsys media.audio_flinger | awk -v p=$pid '$3=="yes" && $4==p {print $(NF-3)}' | head -1)
echo "$1 cpu=$(echo "scale=1; $s/10" | bc) game_fps=$g underruns=$u"
