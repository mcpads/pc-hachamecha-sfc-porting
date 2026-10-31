.include "registers.inc"
.include "io-commands.inc"
.include "common-memmap.inc"

.codebank LOADER_ADDR..(LOADER_ADDR + LOADER_SIZE)

.assert LOADER_ADDR % 2 == 0
.assert CODE_ADDR % 2 == 0

.assert DriverIO__SWITCH_TO_LOADER_PORT == LoaderIO_TransferToLoader__SPINLOCK_PORT
.assert DriverIO__SWITCH_TO_LOADER_BIT == LoaderIO_TransferToLoader__SPINLOCK_SWITCH_TO_LOADER_BIT

.proc start_loader

    .assert PC == LOADER_ADDR

    mov X, #$ff
    mov SP, X

    clrp
.p0

    mov DSPADDR, #DSP_FLG
    mov DSPDATA, #DSP_FLG__SOFT_RESET | DSP_FLG__MUTE_ALL | DSP_FLG__ECHO_DISABLE

    mov CONTROL, #CONTROL__RESET_PORTS_01 | CONTROL__RESET_PORTS_23

    mov A, #DriverIO__MODE_LOADER
    mov DriverIO__MODE_PORT, A

    .assert LoaderIO__LOADER_READY_L == DriverIO__MODE_LOADER
    .assert LoaderIO_InitToScpu__READY_PORT_L + 1 == LoaderIO_InitToScpu__READY_PORT_H

    mov Y, #LoaderIO__LOADER_READY_H
    movw LoaderIO_InitToScpu__READY_PORT_L, YA

    InitSpinloop:
        cmpw YA, LoaderIO_InitToLoader__READY_PORT_L
        bne InitSpinloop

    mov Y, LoaderIO_InitToLoader__DATA_TYPE_PORT
    mov loaderDataType, Y

    .assert LoaderDataType__CODE == 0
    .assert LoaderDataType__SONG_DATA_BIT == 7
    bne DtNotZero

        .assert globalVolume_music + 1 == globalVolume_sfx
        mov A, Y
        movw globalVolume_music, YA

        mov A, #lobyte(CODE_ADDR)
        mov Y, #hibyte(CODE_ADDR)

        bra DtEndIf

    DtNotZero:
        bmi DtIsSong

        mov A, #lobyte(COMMON_DATA_ADDR)
        mov Y, #hibyte(COMMON_DATA_ADDR)

        bra DtEndIf

    DtIsSong:

        movw YA, songPtr
    DtEndIf:

    mov STA_1 + 2, Y
    mov STA_2 + 2, Y

    mov Y, A

    mov LoaderIO_InitAckToScpu__ACK_PORT, #LoaderIO_InitAckToScpu__ACK_VALUE

    mov X, #LoaderIO__LOADER_READY_H

    Loop:

        SpinLoop:
            cmp X, LoaderIO_TransferToLoader__SPINLOCK_PORT
            beq SpinLoop

        mov X, LoaderIO_TransferToLoader__SPINLOCK_PORT

        bmi EndLoop

        mov A, LoaderIO_TransferToLoader__DATA_PORT_L
    STA_1:
        mov $ff00 + Y, A

        mov A, LoaderIO_TransferToLoader__DATA_PORT_H

        mov LoaderIO_TransferToScpu__SPINLOCK_ACK_PORT, X

        inc Y
    STA_2:
        mov $ff00 + Y, A
        inc Y

        bne Loop
            inc STA_1 + 2
            inc STA_2 + 2
            bra  Loop
    EndLoop:

    mov LoaderIO_TransferToScpu__SPINLOCK_ACK_PORT, X

    mov A, X
    and A, #(1 << LoaderIO_TransferToLoader__SPINLOCK_SWITCH_TO_LOADER_BIT)
    bne start_loader

    mov X, loaderDataType
    cmp X, #LoaderDataType__COMMON_DATA
    bne NotCommonData

        mov A, Y
        mov Y, STA_1 + 2
        movw songPtr, YA
    NotCommonData:

    .assert LoaderDataType__SONG_DATA_BIT == 7

    mov A, X
    bpl start_loader

    .assert PC == CODE_ADDR
.endproc
