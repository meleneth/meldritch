local patterns = {}

patterns.Foundation = {
  Kick = {1,0,0,0, .92,0,0,0, 1,0,0,0, .92,0,0,0},
  Snare = {0,0,0,0, .9,0,0,0, 0,0,0,0, 1,0,0,0},
  CHat = {.65,0,.45,0, .72,0,.45,0, .65,0,.45,0, .78,0,.5,0},
  OHat = {0,0,.62,0, 0,0,.7,0, 0,0,.62,0, 0,0,.78,0},
  Clap = {0,0,0,0, .65,0,0,0, 0,0,0,0, .72,0,0,0},
  Perc = {0,0,0,.42, 0,0,.5,0, 0,.38,0,0, 0,0,.55,0},
  Bass = {{step=1,note=36,gate=.72,velocity=.95},{step=4,note=36,gate=.35,velocity=.65},{step=7,note=39,gate=.7,velocity=.8},{step=9,note=36,gate=.72,velocity=.92},{step=12,note=43,gate=.3,velocity=.7},{step=15,note=39,gate=.62,velocity=.82}},
  Lead = {{step=3,note=60,gate=.35,velocity=.6},{step=7,note=63,gate=.4,velocity=.65},{step=11,note=67,gate=.45,velocity=.7},{step=15,note=70,gate=.3,velocity=.6}},
}
patterns.Broken = {
  Kick = {1,0,0,.7, 0,0,.85,0, 0,.72,0,0, .9,0,0,.6}, Snare={0,0,0,0, 0,.8,0,0, 0,0,0,.9, 0,0,.65,0},
  CHat={.4,.3,0,.55, 0,.35,.7,0, .5,0,.35,.6, 0,.4,0,.7}, OHat={0,0,0,0, .6,0,0,0, 0,0,.72,0, 0,0,0,.5},
  Clap={0,0,0,0, 0,.55,0,0, 0,0,0,.68, 0,0,.5,0}, Perc={0,.7,0,0, .45,0,0,.8, 0,0,.6,0, .35,0,0,.9},
  Bass={{step=1,note=36,gate=.3,velocity=.9},{step=4,note=43,gate=.25,velocity=.7},{step=7,note=34,gate=.65,velocity=.85},{step=10,note=39,gate=.3,velocity=.72},{step=14,note=46,gate=.45,velocity=.8}},
  Lead={{step=2,note=67,gate=.2,velocity=.5},{step=6,note=65,gate=.6,velocity=.7},{step=12,note=58,gate=.35,velocity=.65},{step=16,note=70,gate=.2,velocity=.75}},
}
patterns.Acid = {
  Kick={1,0,0,0, 1,0,0,.45, 1,0,0,0, 1,0,.55,0}, Snare={0,0,0,0, .85,0,0,0, 0,0,0,0, .95,0,0,0},
  CHat={.7,0,.5,0, .75,0,.55,0, .72,0,.5,0, .8,0,.6,0}, OHat={0,0,.55,0, 0,0,.6,0, 0,0,.58,0, 0,0,.72,0},
  Clap={0,0,0,0, .5,0,0,0, 0,0,0,0, .6,0,0,0}, Perc={0,0,.4,0, 0,.5,0,.4, 0,0,.55,0, 0,.65,0,.5},
  Bass={{step=1,note=36,gate=.9,velocity=1},{step=3,note=48,gate=.3,velocity=.65},{step=4,note=43,gate=.45,velocity=.85},{step=6,note=39,gate=.7,velocity=.8},{step=8,note=46,gate=.35,velocity=.9},{step=9,note=36,gate=.9,velocity=1},{step=11,note=51,gate=.25,velocity=.7},{step=12,note=43,gate=.5,velocity=.85},{step=14,note=34,gate=.65,velocity=.9},{step=16,note=39,gate=.3,velocity=.8}},
  Lead={{step=5,note=72,gate=.2,velocity=.65},{step=13,note=70,gate=.2,velocity=.6}},
}
patterns.Atmosphere = {
  Kick={.8,0,0,0, 0,0,0,0, .65,0,0,0, 0,0,0,0}, Snare={0,0,0,0, 0,0,0,0, 0,0,0,0, .55,0,0,0},
  CHat={.25,0,0,0, 0,0,.3,0, 0,0,0,0, .35,0,0,0}, OHat={0,0,0,0, .5,0,0,0, 0,0,0,0, 0,0,.6,0},
  Clap={0,0,0,0, 0,0,0,0, 0,0,0,0, .4,0,0,0}, Perc={0,0,0,.45, 0,0,0,0, 0,.5,0,0, 0,0,0,.6},
  Bass={{step=1,note=31,gate=1,velocity=.7},{step=9,note=36,gate=1,velocity=.75}},
  Lead={{step=1,note=60,gate=1,velocity=.7},{step=5,note=67,gate=.9,velocity=.65},{step=9,note=63,gate=1,velocity=.72},{step=13,note=70,gate=.9,velocity=.68}},
}
return patterns
