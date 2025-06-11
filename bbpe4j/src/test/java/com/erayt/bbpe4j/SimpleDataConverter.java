package com.erayt.bbpe4j;

import java.lang.ref.WeakReference;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 4:46 PM
 */
class SimpleDataConverter implements ByteConverter<SimpleData> {

    private static final ThreadLocal<WeakReference<SimpleData>> LOCAL_DATA
            = ThreadLocal.withInitial(() -> new WeakReference<>(new SimpleData()));
    private static final ThreadLocal<WeakReference<byte[]>> LOCAL_BYTES
            = ThreadLocal.withInitial(() -> new WeakReference<>(new byte[512]));

    @Override
    public SimpleData convert(byte[] bytes, int offset) {
        SimpleData data = LOCAL_DATA.get().get();
        if (data == null) {
            LOCAL_DATA.remove();
            data = new SimpleData();
            LOCAL_DATA.set(new WeakReference<>(data));
        }
        data.setVal1(Bytes.readInt(bytes, offset));
        data.setVal2(Bytes.readLong(bytes, offset + 8));
        data.setVal3(Bytes.readDouble(bytes, offset + 16));
        data.setVal4(Bytes.readString(bytes, offset + 24, 16));
        return data;
    }

    @Override
    public byte[] convert(SimpleData data) {
        byte[] bytes = LOCAL_BYTES.get().get();
        if (bytes == null) {
            LOCAL_BYTES.remove();
            bytes = new byte[512];
            LOCAL_BYTES.set(new WeakReference<>(bytes));
        }
        Bytes.writeInt(bytes, 0, data.getVal1());
        Bytes.writeLong(bytes, 8, data.getVal2());
        Bytes.writeDouble(bytes, 16, data.getVal3());
        Bytes.writeString(bytes, 24, 16, data.getVal4());
        return bytes;
    }
}
