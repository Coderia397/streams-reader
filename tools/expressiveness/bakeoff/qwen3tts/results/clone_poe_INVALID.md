INVALID (2026-10-31): run 2 poe_f clone babbles (WER seg 19.5 %, sent 55.6 %, results/check_run2.json); the reference clip ran 0.25 s into the next word, so the prompt text did not match the audio. Do not use these scores. See notes/25.

| voice | passage | F0 med Hz | F0 sd st | F0 range st | slope sd st/s | F0 level sd st | energy p90-p10 dB | energy level sd dB | pauses | pause ms med (p10-p90) | ln-pause sd | syl/s | rate sd | EI | % of human |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| qwen_clone_poe_f_seg | 1-suspense | 226 | 4.5 | 13.7 | 7.3 | 3.39 | 22.1 | 4.03 | 23 | 340 (186-838) | 0.63 | 4.0 | 0.87 | 2.06 | 167 |
| qwen_clone_poe_f_sent | 1-suspense | 216 | 4.4 | 14.5 | 4.1 | 3.39 | 20.9 | 2.16 | 23 | 360 (236-732) | 0.46 | 4.2 | 0.98 | 1.39 | 129 |
| qwen_clone_poe_f_seg | 2-dialogue | 247 | 4.5 | 14.4 | 5.3 | 3.36 | 24.0 | 3.66 | 41 | 230 (160-700) | 0.55 | 3.7 | 1.01 | 2.07 | 167 |
| qwen_clone_poe_f_sent | 2-dialogue | 217 | 4.3 | 13.6 | 6.2 | 2.92 | 21.3 | 1.67 | 25 | 440 (246-844) | 0.52 | 3.7 | 0.91 | 1.41 | 130 |
| qwen_clone_poe_f_seg | 3-reflective | 246 | 4.1 | 13.3 | 2.5 | 2.77 | 21.8 | 2.56 | 16 | 285 (165-755) | 0.59 | 3.7 | 0.92 | 1.48 | 134 |
| qwen_clone_poe_f_sent | 3-reflective | 215 | 3.9 | 12.7 | 7.4 | 1.53 | 20.9 | 1.32 | 14 | 425 (313-752) | 0.42 | 4.1 | 0.54 | 0.63 | 85 |

| voice (mean of 3 passages) | F0 sd st | F0 range st | slope sd | F0 level sd | energy dB | energy level sd | ln-pause sd | rate sd | EI | % of human |
|---|---|---|---|---|---|---|---|---|---|---|
| qwen_clone_poe_f_seg | 4.3 | 13.8 | 5.0 | 3.17 | 22.6 | 3.42 | 0.59 | 0.94 | 1.87 | 156 |
| qwen_clone_poe_f_sent | 4.2 | 13.6 | 5.9 | 2.61 | 21.0 | 1.72 | 0.47 | 0.81 | 1.14 | 114 |
