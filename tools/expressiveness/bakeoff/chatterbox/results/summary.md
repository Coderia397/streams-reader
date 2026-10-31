| condition | model | voice | EI 1/2/3 | % of human 1/2/3 | % mean | WER | UTMOS | s/syll |
|---|---|---|---|---|---|---|---|---|
| cb_orig_e50_female | orig | female | 0.76/0.29/0.59 | 93/66/83 | 80 | 2.0 % | 4.28 | 0.245 |
| cb_orig_e70_female | orig | female | 0.59/0.86/0.52 | 83/98/79 | 87 | 1.8 % | 4.197 | 0.233 |
| cb_orig_e90_female | orig | female | 1.59/1.43/0.64 | 140/131/86 | 119 | 2.1 % | 3.92 | 0.256 |
| cb_orig_dir_female | orig | female | 0.97/1.05/0.55 | 105/109/81 | 98 | 3.7 % | 4.057 | 0.296 |
| cb_orig_dir_female_dp | orig | female | 1.11/1.15/0.67 | 112/114/87 | 105 | 3.7 % | 4.057 | 0.296 |
| cb_orig_e50_male | orig | male | 1.14/1.85/1.27 | 114/155/122 | 130 | 1.2 % | 4.437 | 0.306 |
| cb_orig_e70_male | orig | male | 0.97/1.95/1.11 | 105/161/112 | 126 | 0.6 % | 4.447 | 0.297 |
| cb_orig_e90_male | orig | male | 1.99/2.38/0.89 | 163/185/100 | 149 | 1.7 % | 4.38 | 0.316 |
| cb_orig_dir_male | orig | male | 1.37/1.49/1.10 | 128/134/112 | 125 | 2.0 % | 4.27 | 0.387 |
| cb_orig_dir_male_dp | orig | male | 1.50/1.57/1.25 | 135/139/120 | 131 | 2.0 % | 4.27 | 0.387 |
| cb_turbo_female | turbo | female | 0.95/0.97/0.29 | 103/105/66 | 91 | 2.0 % | 4.14 | 0.258 |
| cb_turbo_tags_female | turbo | female | 0.45/0.94/-0.02 | 75/103/48 | 75 | 4.0 % | 4.08 | 0.297 |
| cb_turbo_tags_female_dp | turbo | female | 0.55/0.99/0.05 | 80/106/52 | 79 | 4.0 % | 4.08 | 0.297 |
| cb_turbo_male | turbo | male | 0.70/1.41/0.80 | 89/129/95 | 104 | 1.2 % | 4.403 | 0.317 |
| cb_turbo_tags_male | turbo | male | 0.27/1.02/0.70 | 65/108/89 | 87 | 1.4 % | 4.253 | 0.357 |
| cb_turbo_tags_male_dp | turbo | male | 0.38/1.11/0.83 | 70/113/96 | 93 | 1.4 % | 4.253 | 0.357 |
| cb_mtl3_female | mtl | female | 0.35/0.49/-0.47 | 69/77/22 | 56 | 1.2 % | 4.247 | 0.24 |
| cb_mtl3_dir_female | mtl | female | 0.47/0.28/0.35 | 76/65/69 | 70 | 4.3 % | 4.25 | 0.309 |
| cb_mtl3_dir_female_dp | mtl | female | 0.57/0.40/0.52 | 82/72/79 | 77 | 4.3 % | 4.25 | 0.309 |
| cb_mtl3_male | mtl | male | 0.63/1.76/0.58 | 85/150/82 | 106 | 0.6 % | 4.42 | 0.314 |
| cb_mtl3_dir_male | mtl | male | 0.56/1.35/0.52 | 81/126/79 | 95 | 2.2 % | 4.3 | 0.442 |
| cb_mtl3_dir_male_dp | mtl | male | 0.66/1.50/0.70 | 87/135/89 | 104 | 2.2 % | 4.3 | 0.442 |

| baseline | passage | WER | UTMOS |
|---|---|---|---|
| af_heart | 1-suspense | 0.0 | 4.52 |
| af_heart | 2-dialogue | 0.0 | 4.49 |
| af_heart | 3-reflective | 0.017 | 4.51 |
| am_michael | 1-suspense | 0.008 | 4.36 |
| am_michael | 2-dialogue | 0.0 | 4.25 |
| am_michael | 3-reflective | 0.017 | 4.35 |
| bf_emma | 1-suspense | 0.0 | 4.33 |
| bf_emma | 2-dialogue | 0.009 | 4.34 |
| bf_emma | 3-reflective | 0.0 | 4.35 |
| bm_george | 1-suspense | 0.0 | 4.2 |
| bm_george | 2-dialogue | 0.009 | 4.22 |
| bm_george | 3-reflective | 0.0 | 4.21 |
| human | 1-suspense | None | 3.55 |
| human | 2-dialogue | None | 2.46 |
| human | 3-reflective | None | 4.1 |

| model | load s | weights GB | peak VRAM GB | reserved GB | items | GPU s | audio s | RTF |
|---|---|---|---|---|---|---|---|---|
| orig | 12.3 | 2.99 | 3.88 | 4.81 | 158 | 1001.9 | 1022.0 | 0.98 |
| turbo | 11.2 | 2.61 | 3.48 | 4.44 | 110 | 237.3 | 525.0 | 0.452 |
| mtl | 16.1 | 3.01 | 3.87 | 4.5 | 110 | 565.8 | 547.2 | 1.034 |

References: {"female": {"utmos": 4.043, "asr": "It is a truth universally acknowledged that a single man in possession of a good fortune must be in want of a wife. However little known the feelings or views of such a man may be, on his first entering a neighborhood,"}, "male": {"utmos": 4.29, "asr": "These small waves raised by the evening wind are as remote from storm as the smooth reflecting surface."}}

Flags:
- cb_orig_e50_female/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years outside the sea lay fla' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e70_female/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years outside the sea lay fla' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e70_female/1-suspense/01.flac: clipped | said: 'then the gulls rose all at once without a sound he set the pipe down somewhere beneath him on the iron stairs a step cre' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_orig_e90_female/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years outside the sea lay fla' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e90_female/1-suspense/01.flac: clipped | said: 'then the gulls rose all at once without a sound he set the pike down somewhere beneath him on the iron stairs a step cre' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_orig_e90_female/1-suspense/02.flac: long 2.1x median, clipped | said: 'He reached for the lantern. His hand was shaking. The door handle began very slowly to turn. "\'Oh!\'' | text: 'He reached for the lantern. His hand was shaking. The door handle began, very sl'
- cb_orig_dir_female/1-suspense/01.flac: clipped | said: 'Outside the sea lay flat and silver under a pale sky, and the gulls had settled on the rocks below the lamproom.' | text: 'Outside, the sea lay flat and silver under a pale sky, and the gulls had settled'
- cb_orig_dir_female/1-suspense/02.flac: clipped | said: 'He wound the clock, trimmed the wick, and sat down by the window with his pipe.' | text: 'He wound the clock, trimmed the wick, and sat down by the window with his pipe.'
- cb_orig_dir_female/1-suspense/11.flac: long 2.3x median, WER 50% | said: 'The door handle began very slowly to turn, very slowly to turn.' | text: 'The door handle began, very slowly, to turn.'
- cb_orig_e50_female/2-dialogue/05.flac: clipped | said: 'i think he began and stopped he glanced at the window i think some one has been following me for a week perhaps longer o' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_orig_e70_female/2-dialogue/03.flac: clipped | said: 'He looked down at the floor. "\'I know. I\'m sorry. I\'m not sure I can explain it. Not properly.\'' | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_orig_e90_female/2-dialogue/01.flac: clipped | said: '"\'Oh, certainly,\' said Tom. "\'I was at the opera with the Queen,\' she sends her warmest regards.' | text: '"Oh, certainly," said Tom. "I was at the opera. With the Queen. She sends her wa'
- cb_orig_e90_female/2-dialogue/02.flac: clipped | said: '"\'Don\'t you dare make a joke of this. I waited up until two in the morning, Tom. Two!\'' | text: '"Don\'t you dare make a joke of this! I waited up until two in the morning, Tom. '
- cb_orig_e90_female/2-dialogue/03.flac: clipped | said: "He looked down at the floor. I know. I'm sorry. I'm not sure I can explain it. Not properly." | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_orig_e90_female/2-dialogue/04.flac: clipped | said: 'then try she said more quietly now ow' | text: '"Then try," she said, more quietly now.'
- cb_orig_e90_female/2-dialogue/05.flac: clipped | said: 'i think he began and stopped he glanced at the window i think some one has been following me for a week perhaps longer o' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_orig_dir_female/2-dialogue/03.flac: WER 20%, clipped | said: '"\'Ooh, I was at the opera.\'' | text: '"I was at the opera.'
- cb_orig_dir_female/2-dialogue/05.flac: WER 20% | said: 'SHE SENS HER WARMEST REGARDS.' | text: 'She sends her warmest regards."'
- cb_orig_dir_female/2-dialogue/08.flac: long 2.6x median, WER 100% | said: 'Digadoo!' | text: 'Two!"'
- cb_orig_dir_female/2-dialogue/10.flac: long 2.5x median | said: 'Hmm, I know.' | text: '"I know.'
- cb_orig_dir_female/2-dialogue/11.flac: long 2.8x median, clipped | said: "I'm sorry." | text: "I'm sorry."
- cb_orig_e50_female/3-reflective/00.flac: clipped | said: 'there is a particular kind of quiet that comes only in the last hour before dawn the house still ticks and the wind stil' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e50_female/3-reflective/01.flac: clipped | said: 'i used to fill that hour i read i wrote letters i made long lists of everything i meant to do now more often i simply si' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_e50_female/3-reflective/02.flac: clipped | said: 'what i have learned if i have learned anything at all is this the things that matter most rarely announce themselves the' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_e70_female/3-reflective/00.flac: clipped | said: 'there is a particular kind of quiet that comes only in the last hour before dawn the house still ticks and the wind stil' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e70_female/3-reflective/01.flac: clipped | said: 'i used to fill that hour i read i wrote letters i made long lists of everything i meant to do now more often i simply si' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_e70_female/3-reflective/02.flac: clipped | said: 'what i have learned if i have learned anything at all is this the things that matter most rarely announce themselves the' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_e90_female/3-reflective/00.flac: clipped | said: 'there is a particular kind of quiet that comes only in the last hour before dawn the house still ticks and the wind stil' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e90_female/3-reflective/01.flac: clipped | said: 'i used to fill that hour i read i wrote letters i made long lists of everything i meant to do now more often i simply si' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_dir_female/3-reflective/07.flac: clipped | said: 'what i have learned if i have learned anything at all is this the things that matter most rarely announce themselves' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_dir_female/3-reflective/08.flac: WER 67% | said: 'Clay arrives slowly.' | text: 'They arrive slowly.'
- cb_orig_dir_female/3-reflective/10.flac: clipped | said: 'and they ask only that we be still enough to notice' | text: 'And they ask only that we be still enough to notice.'
- cb_orig_e50_male/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years outside the sea lay fla' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e50_male/1-suspense/01.flac: clipped | said: 'then the gulls rose all at once without a sound he set the pipe down somewhere beneath him on the iron stairs a step cre' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_orig_e50_male/1-suspense/02.flac: clipped | said: 'he reached for the lantern his hand was shaking the door handle began very slowly to turn' | text: 'He reached for the lantern. His hand was shaking. The door handle began, very sl'
- cb_orig_e70_male/1-suspense/00.flac: clipped | said: 'The lighthouse keeper finished his supper at seven, as he had done every evening for eleven years. Outside the sea lay f' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e70_male/1-suspense/01.flac: clipped | said: 'then the gulls rose all at once without a sound he set the pipe down somewhere beneath him on the iron stairs a step cre' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_orig_e70_male/1-suspense/02.flac: clipped | said: 'He reached for the lantern. His hand was shaking. The door handle began very slowly to turn.' | text: 'He reached for the lantern. His hand was shaking. The door handle began, very sl'
- cb_orig_e90_male/1-suspense/00.flac: clipped | said: 'The lighthouse keeper finished his supper at seven, as he had done every evening for eleven years. Outside, the sea lay ' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_e90_male/1-suspense/01.flac: clipped | said: 'Then the gulls rose all at once, without a sound. He set the pipe down. Somewhere beneath him, on the iron stairs, a ste' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_orig_e90_male/1-suspense/02.flac: clipped | said: 'He reached for the lantern. His hand was shaking. The door handle began very slowly to turn.' | text: 'He reached for the lantern. His hand was shaking. The door handle began, very sl'
- cb_orig_dir_male/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_orig_dir_male/1-suspense/02.flac: clipped | said: 'he wound the clock trimmed the wick and sat down by the window with his pipe' | text: 'He wound the clock, trimmed the wick, and sat down by the window with his pipe.'
- cb_orig_dir_male/1-suspense/03.flac: clipped | said: 'then the gulls rose all at once without a sound' | text: 'Then the gulls rose, all at once, without a sound.'
- cb_orig_dir_male/1-suspense/05.flac: clipped | said: 'Somewhere beneath him, on the iron stairs, a step creaked.' | text: 'Somewhere beneath him, on the iron stairs, a step creaked.'
- cb_orig_dir_male/1-suspense/06.flac: clipped | said: 'Then another.' | text: 'Then another.'
- cb_orig_dir_male/1-suspense/08.flac: clipped | said: 'The steps were slow and heavy and they were getting closer.' | text: 'The steps were slow and heavy, and they were getting closer.'
- cb_orig_dir_male/1-suspense/11.flac: clipped | said: 'The door handle began very slowly to turn.' | text: 'The door handle began, very slowly, to turn.'
- cb_orig_e50_male/2-dialogue/01.flac: clipped | said: 'Oh, certainly, said Tom. I was at the opera. With the Queen, she sends her warmest regards.' | text: '"Oh, certainly," said Tom. "I was at the opera. With the Queen. She sends her wa'
- cb_orig_e50_male/2-dialogue/03.flac: clipped | said: "He looked down at the floor. I know. I'm sorry. I'm not sure I can explain it. Not properly." | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_orig_e50_male/2-dialogue/05.flac: clipped | said: 'i think he began and stopped he glanced at the window i think someone has been following me for a week perhaps longer or' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_orig_e70_male/2-dialogue/01.flac: clipped | said: '"\'Oh, certainly,\' said Tom. "\'I was at the opera. "\'With the Queen. "\'She sends her warmest regards.\'' | text: '"Oh, certainly," said Tom. "I was at the opera. With the Queen. She sends her wa'
- cb_orig_e70_male/2-dialogue/02.flac: clipped | said: "Don't you dare make a joke of this. I waited up until two in the morning, Tom. Two!" | text: '"Don\'t you dare make a joke of this! I waited up until two in the morning, Tom. '
- cb_orig_e70_male/2-dialogue/03.flac: clipped | said: "He looked down at the floor. I know. I'm sorry. I'm not sure I can explain it. Not properly." | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_orig_e70_male/2-dialogue/05.flac: clipped | said: 'I think, he began and stopped. He glanced at the window. I think someone has been following me for a week, perhaps longe' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_orig_e90_male/2-dialogue/00.flac: clipped | said: '"\'Are you going to tell me where you were last night?\' Martha asked, folding her arms.' | text: '"Are you going to tell me where you were last night?" Martha asked, folding her '
- cb_orig_e90_male/2-dialogue/01.flac: clipped | said: 'Oh, certainly, said Tom. I was at the opera with the Queen. She sends her warmest regards.' | text: '"Oh, certainly," said Tom. "I was at the opera. With the Queen. She sends her wa'
- cb_orig_e90_male/2-dialogue/02.flac: clipped | said: "Don't you dare make a joke of this. I waited up until two in the morning, Tom. Two!" | text: '"Don\'t you dare make a joke of this! I waited up until two in the morning, Tom. '
- cb_orig_e90_male/2-dialogue/03.flac: clipped | said: "He looked down at the floor. I know. I'm sorry. I'm not sure I can explain it. Not properly." | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_orig_e90_male/2-dialogue/04.flac: long 1.7x median, clipped | said: 'Then try, she said, more quietly now.' | text: '"Then try," she said, more quietly now.'
- cb_orig_e90_male/2-dialogue/05.flac: clipped | said: 'i think he began and stopped he glanced at the window i think someone has been following me for a week perhaps longer or' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_orig_dir_male/2-dialogue/01.flac: WER 20% | said: 'MARTHA ASKED, FOLDING HER ARMS. YES.' | text: 'Martha asked, folding her arms.'
- cb_orig_dir_male/2-dialogue/06.flac: clipped | said: "Don't you dare make a joke of this." | text: '"Don\'t you dare make a joke of this!'
- cb_orig_dir_male/2-dialogue/08.flac: long 3.2x median | said: 'Two.' | text: 'Two!"'
- cb_orig_dir_male/2-dialogue/10.flac: long 2.8x median, clipped | said: 'I know.' | text: '"I know.'
- cb_orig_dir_male/2-dialogue/12.flac: clipped | said: "I'm not sure I can explain it. Not properly." | text: 'I\'m not sure I can explain it, not properly."'
- cb_orig_dir_male/2-dialogue/16.flac: clipped | said: 'I think someone has been following me.' | text: '"I think someone has been following me.'
- cb_orig_dir_male/2-dialogue/19.flac: clipped | said: "I honestly don't know anymore." | text: 'I honestly don\'t know anymore."'
- cb_orig_e50_male/3-reflective/00.flac: clipped | said: 'there is a particular kind of quiet that comes only in the last hour before dawn the house still ticks and the wind stil' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e50_male/3-reflective/01.flac: clipped | said: 'I used to fill that hour. I read. I wrote letters. I made long lists of everything I meant to do. Now, more often, I sim' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_e50_male/3-reflective/02.flac: clipped | said: 'What I have learned, if I have learned anything at all, is this. The things that matter most rarely announce themselves.' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_e70_male/3-reflective/00.flac: clipped | said: 'There is a particular kind of quiet that comes only in the last hour before dawn. The house still ticks and the wind sti' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e70_male/3-reflective/01.flac: clipped | said: 'i used to fill that hour i read i wrote letters i made long lists of everything i meant to do now more often i simply si' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_e70_male/3-reflective/02.flac: clipped | said: 'What I have learned, if I have learned anything at all, is this, the things that matter most rarely announce themselves.' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_e90_male/3-reflective/00.flac: clipped | said: 'there is a particular kind of quiet that comes only in the last hour before dawn the house still ticks and the wind stil' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_e90_male/3-reflective/01.flac: clipped | said: 'I used to fill that hour. I read, I wrote letters, I made long lists of everything I meant to do. Now, more often, I sim' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_orig_e90_male/3-reflective/02.flac: clipped | said: 'what i have learned if i have learned anything at all is this the things that matter most rarely announce themselves the' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_dir_male/3-reflective/00.flac: clipped | said: 'There is a particular kind of quiet that comes only in the last hour before dawn.' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_orig_dir_male/3-reflective/07.flac: clipped | said: 'What I have learned, if I have learned anything at all, is this. The things that matter most rarely announce themselves.' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_orig_dir_male/3-reflective/09.flac: long 1.9x median | said: 'They wait.' | text: 'They wait.'
- cb_turbo_tags_female/1-suspense/04.flac: WER 20% | said: 'You set the pipe down.' | text: 'He set the pipe down.'
- cb_turbo_tags_female/1-suspense/06.flac: long 1.7x median | said: 'Then another.' | text: '[fear] [gasp] Then another.'
- cb_turbo_tags_female/1-suspense/10.flac: long 1.8x median | said: 'His hand was shaking.' | text: '[fear] [gasp] His hand was shaking.'
- cb_turbo_tags_female/2-dialogue/02.flac: long 1.6x median | said: '"\'Oh, certainly,\' said Tom.' | text: '[happy] [chuckle] "Oh, certainly," said Tom.'
- cb_turbo_tags_female/2-dialogue/08.flac: long 3.9x median | said: '2' | text: '[angry] Two!"'
- cb_turbo_tags_female/2-dialogue/09.flac: WER 33% | said: 'You lift down at the floor.' | text: 'He looked down at the floor.'
- cb_turbo_tags_female/2-dialogue/10.flac: long 2.7x median | said: 'I know.' | text: '[sigh] "I know.'
- cb_turbo_tags_female/2-dialogue/15.flac: WER 20% | said: 'You glanced at the window.' | text: 'He glanced at the window.'
- cb_turbo_tags_female/2-dialogue/19.flac: WER 40% | said: "I honestly don't know any more." | text: '[fear] I honestly don\'t know anymore."'
- cb_turbo_tags_female/3-reflective/08.flac: WER 33% | said: 'They arrived slowly.' | text: 'They arrive slowly.'
- cb_turbo_tags_female/3-reflective/09.flac: long 1.9x median | said: 'They wait.' | text: 'They wait.'
- cb_turbo_tags_male/1-suspense/06.flac: WER 50% | said: 'than another.' | text: '[fear] [gasp] Then another.'
- cb_turbo_male/2-dialogue/03.flac: silence 1.4 s | said: 'He looked down at the floor. "\'I know. I\'m sorry. I\'m not sure I can explain it. Not properly.\'' | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_turbo_tags_male/2-dialogue/08.flac: long 5.9x median, WER 100% | said: 'Two. Two.' | text: '[angry] Two!"'
- cb_turbo_tags_male/2-dialogue/10.flac: long 2.1x median | said: 'I know.' | text: '[sigh] "I know.'
- cb_turbo_tags_male/3-reflective/09.flac: long 1.8x median | said: 'They wait.' | text: 'They wait.'
- cb_mtl3_female/1-suspense/00.flac: clipped | said: 'the lighthouse keeper finished his supper at seven as he had done every evening for eleven years outside the sea lay fla' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_mtl3_dir_female/1-suspense/09.flac: WER 40% | said: 'you reach for the lantern.' | text: 'He reached for the lantern.'
- cb_mtl3_female/2-dialogue/00.flac: clipped | said: '"\'Are you going to tell me where you were last night?\' Martha asked, folding her arms.' | text: '"Are you going to tell me where you were last night?" Martha asked, folding her '
- cb_mtl3_female/2-dialogue/03.flac: clipped | said: 'He looked down at the floor. "\'I know. I\'m sorry. I\'m not sure I can explain it. Not properly.\'' | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_mtl3_female/2-dialogue/05.flac: clipped | said: '"\'I think,\' he began, and stopped. He glanced at the window. "\'I think someone has been following me. For a week, perhap' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_mtl3_dir_female/2-dialogue/08.flac: long 3.1x median | said: '2.' | text: 'Two!"'
- cb_mtl3_dir_female/2-dialogue/10.flac: long 6.7x median, WER 350%, clipped | said: 'I know. I know my stock. What an APC.' | text: '"I know.'
- cb_mtl3_female/3-reflective/01.flac: clipped | said: 'I used to fill that hour. I read, I wrote letters, I made long lists of everything I meant to do. Now, more often, I sim' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_mtl3_female/3-reflective/02.flac: clipped | said: 'what i have learned if i have learned anything at all is this the things that matter most rarely announce themselves the' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_mtl3_dir_female/3-reflective/09.flac: long 2.1x median | said: 'they wait' | text: 'They wait.'
- cb_mtl3_male/1-suspense/00.flac: clipped | said: 'The lighthouse keeper finished his supper at seven, as he had done every evening for eleven years. Outside the sea lay f' | text: 'The lighthouse keeper finished his supper at seven, as he had done every evening'
- cb_mtl3_male/1-suspense/01.flac: clipped | said: 'Then the gulls rose. All at once, without a sound, he set the pipe down. Somewhere beneath him on the iron stairs a step' | text: 'Then the gulls rose, all at once, without a sound. He set the pipe down. Somewhe'
- cb_mtl3_male/1-suspense/02.flac: clipped | said: 'He reached for the lantern. His hand was shaking. The door handle began very slowly to turn.' | text: 'He reached for the lantern. His hand was shaking. The door handle began, very sl'
- cb_mtl3_dir_male/1-suspense/02.flac: clipped | said: 'He wound the clock, trimmed the wick, and sat down by the window with his pipe.' | text: 'He wound the clock, trimmed the wick, and sat down by the window with his pipe.'
- cb_mtl3_dir_male/1-suspense/03.flac: clipped | said: 'Then the gulls rose, all at once without a sound.' | text: 'Then the gulls rose, all at once, without a sound.'
- cb_mtl3_dir_male/1-suspense/05.flac: WER 20% | said: 'Somewhere beneath him on the iron stairs a stepcrete.' | text: 'Somewhere beneath him, on the iron stairs, a step creaked.'
- cb_mtl3_male/2-dialogue/03.flac: clipped | said: 'He looked down at the floor. "\'I know. I\'m sorry. I\'m not sure I can explain it, not properly.\'' | text: 'He looked down at the floor. "I know. I\'m sorry. I\'m not sure I can explain it, '
- cb_mtl3_male/2-dialogue/05.flac: clipped | said: 'I think, he began, and stopped. He glanced at the window. I think someone has been following me, for a week, perhaps lon' | text: '"I think," he began, and stopped. He glanced at the window. "I think someone has'
- cb_mtl3_dir_male/2-dialogue/08.flac: long 5.7x median, WER 100% | said: 'Two. Two.' | text: 'Two!"'
- cb_mtl3_male/3-reflective/00.flac: clipped | said: 'There is a particular kind of quiet that comes only in the last hour before dawn. The house still ticks, and the wind st' | text: 'There is a particular kind of quiet that comes only in the last hour before dawn'
- cb_mtl3_male/3-reflective/01.flac: clipped | said: 'I used to fill that hour. I read, I wrote letters, I made long lists of everything I meant to do. Now, more often, I sim' | text: 'I used to fill that hour. I read, I wrote letters, I made long lists of everythi'
- cb_mtl3_dir_male/3-reflective/01.flac: clipped | said: 'The house still ticks, and the wind still moves in the chimney.' | text: 'The house still ticks, and the wind still moves in the chimney.'
- cb_mtl3_dir_male/3-reflective/07.flac: clipped | said: 'What I have learned, if I have learned anything at all, is this the things that matter most rarely announce themselves.' | text: 'What I have learned, if I have learned anything at all, is this: the things that'
- cb_mtl3_dir_male/3-reflective/09.flac: long 1.8x median | said: 'They wait.' | text: 'They wait.'
