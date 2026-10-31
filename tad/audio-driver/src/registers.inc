    DSP_MVOL_L  = $0c
    DSP_MVOL_R  = $1c
    DSP_EVOL_L  = $2c
    DSP_EVOL_R  = $3c
    DSP_KON     = $4c
    DSP_KOFF    = $5c
    DSP_FLG     = $6c
    DSP_ENDX    = $7c
    DSP_EFB     = $0d
    DSP_PMON    = $2d
    DSP_NON     = $3d
    DSP_EON     = $4d
    DSP_DIR     = $5d
    DSP_ESA     = $6d
    DSP_EDL     = $7d

    DSP_C0      = $0f
    DSP_C1      = $1f
    DSP_C2      = $2f
    DSP_C3      = $3f
    DSP_C4      = $4f
    DSP_C5      = $5f
    DSP_C6      = $6f
    DSP_C7      = $7f

    VxVOL_L     = $0
    VxVOL_R     = $1
    VxPITCH_L   = $2
    VxPITCH_H   = $3
    VxSRCN      = $4
    VxADSR1     = $5
    VxADSR2     = $6
    VxGAIN      = $7

    VxENVX      = $8
    VxOUTX      = $9

    DSP_FLG__SOFT_RESET         = $80
    DSP_FLG__MUTE_ALL           = $40
    DSP_FLG__ECHO_DISABLE       = $20
    DSP_FLG__NOISE_FREQ_MASK    = $1f

    VxADSR1_ENABLE_BIT  = 7

    CONTROL     = $f1
    DSPADDR     = $f2
    DSPDATA     = $f3

    CPUIO0      = $f4
    CPUIO1      = $f5
    CPUIO2      = $f6
    CPUIO3      = $f7

    T0TARGET    = $fa
    T1TARGET    = $fb
    T2TARGET    = $fc

    T0OUT       = $fd
    T1OUT       = $fe
    T2OUT       = $ff

    CONTROL__ENABLE_TIMER_0 = 1 << 0
    CONTROL__ENABLE_TIMER_1 = 1 << 1
    CONTROL__ENABLE_TIMER_2 = 1 << 2
    CONTROL__RESET_PORTS_01 = 1 << 4
    CONTROL__RESET_PORTS_23 = 1 << 5
    CONTROL__ENABLE_IPL     = 1 << 7

    TIMER_0_HZ =  8000
    TIMER_1_HZ =  8000
    TIMER_2_HZ = 64000
