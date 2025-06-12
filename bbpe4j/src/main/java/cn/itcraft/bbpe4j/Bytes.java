package cn.itcraft.bbpe4j;

import sun.misc.Unsafe;

import java.nio.charset.StandardCharsets;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 4:44 PM
 */
public final class Bytes {

    private static final Unsafe UNSAFE = UnsafeUtil.getTheUnsafe();

    public static int readInt(byte[] bytes, int offset) {
        return (int) readLong(bytes, offset);
    }

    public static long readLong(byte[] bytes, int offset) {
        long memOffset = Unsafe.ARRAY_BYTE_BASE_OFFSET + ((long) offset) * Unsafe.ARRAY_BYTE_INDEX_SCALE;
        return UNSAFE.getLong(bytes, memOffset);
    }

    public static double readDouble(byte[] bytes, int offset) {
        long memOffset = Unsafe.ARRAY_BYTE_BASE_OFFSET + ((long) offset) * Unsafe.ARRAY_BYTE_INDEX_SCALE;
        return UNSAFE.getDouble(bytes, memOffset);
    }

    public static String readString(byte[] bytes, int offset, int length) {
        return new String(bytes, offset, length);
    }

    public static void writeInt(byte[] bytes, int offset, int value) {
        writeLong(bytes, offset, value);
    }

    public static void writeLong(byte[] bytes, int offset, long value) {
        long memOffset = Unsafe.ARRAY_BYTE_BASE_OFFSET + ((long) offset) * Unsafe.ARRAY_BYTE_INDEX_SCALE;
        UNSAFE.putLong(bytes, memOffset, value);
    }

    public static void writeDouble(byte[] bytes, int offset, double value) {
        long memOffset = Unsafe.ARRAY_BYTE_BASE_OFFSET + ((long) offset) * Unsafe.ARRAY_BYTE_INDEX_SCALE;
        UNSAFE.putDouble(bytes, memOffset, value);
    }

    public static void writeString(byte[] bytes, int offset, int length, String value) {
        byte[] data = value.getBytes(StandardCharsets.UTF_8);
        int len = Math.min(data.length, length);
        System.arraycopy(data, 0, bytes, offset, len);
    }
}
