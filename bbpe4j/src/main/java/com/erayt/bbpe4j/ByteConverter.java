package com.erayt.bbpe4j;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 4:31 PM
 */
public interface ByteConverter<T> {
    T convert(byte[] bytes, int offset);

    byte[] convert(T data);
}
