package org.teavm.interop;

/**
 * Wasm linear-memory helpers referenced by wit-bindgen {@code teavm-java} output.
 * <p>
 * The TeaVM compiler supplies the real implementation for Wasm GC targets; plain {@code javac}
 * only needs the signatures so the generated {@code GameCore} class compiles.
 */
public final class Memory {

    private Memory() {}

    public static native void getBytes(Address address, byte[] target, int offset, int length);

    public static native Address malloc(int sizeBytes, int align);

    public static native void putBytes(Address address, byte[] source, int offset, int length);

    public static native void free(Address address, int sizeBytes, int align);
}
