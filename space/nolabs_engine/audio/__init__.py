"""Post-processing between a model's float waveform and the WebM/Opus file stored in R2 (notes/09, Audio):
`silence` trims model silence and appends the boundary pause, `loudness` plans one static gain per batch, `codec` drives
ffmpeg, and `pipeline` runs a whole batch through them."""
