#language slang 2026

module _debug_common;

public struct PackedFields {
    public uint32_t data;

    public property bool ignore_depth {
        get { return (data & 0x1) != 0; }
    }
}
