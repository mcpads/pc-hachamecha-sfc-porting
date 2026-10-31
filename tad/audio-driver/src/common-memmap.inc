LOADER_ADDR = $200
LOADER_SIZE =  116

SHARED_ZEROPAGE_ADDR = $00eb

STACK_SIZE = $20
STACK_BOTTOM_ADDR = $200 - STACK_SIZE

BC_CHANNEL_STACK_SIZE = 21
BC_CHANNEL_STACK_OFFSET = 6

LAST_VAR_PAGE_SIZE = (BC_CHANNEL_STACK_SIZE + 11) * 10

COMMON_DATA_ADDR = $1100 - 24

CODE_ADDR = $200 + LOADER_SIZE
CODE_END_ADDR = COMMON_DATA_ADDR - LAST_VAR_PAGE_SIZE

.varbank zeropage           $0000..SHARED_ZEROPAGE_ADDR
.varbank loader_zeropage    SHARED_ZEROPAGE_ADDR..$00f0
.varbank firstpage          $0100..STACK_BOTTOM_ADDR
.varbank stack              STACK_BOTTOM_ADDR..$0200
.varbank lastvarpage        CODE_END_ADDR..COMMON_DATA_ADDR

.vars loader_zeropage

    globalVolume_music : u8
    globalVolume_sfx : u8

    songPtr :  ptr

    loaderDataType : u8
.endvars
