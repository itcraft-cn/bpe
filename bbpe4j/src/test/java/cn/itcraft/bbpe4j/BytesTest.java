package cn.itcraft.bbpe4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * @author Helly Guo
 * <p>
 * Created on 11/16/23 5:12 PM
 */
public class BytesTest {

    private static final Logger LOGGER = LoggerFactory.getLogger(BytesTest.class);

    @Test
    public void test() {
        byte[] bytes = new byte[512];
        Bytes.writeInt(bytes, 0, 3);
        Bytes.writeLong(bytes, 8, 7L);
        Bytes.writeDouble(bytes, 16, 1.2345D);
        Bytes.writeString(bytes, 24, 7, "hello");
        int value1 = Bytes.readInt(bytes, 0);
        long value2 = Bytes.readLong(bytes, 8);
        double value3 = Bytes.readDouble(bytes, 16);
        String value4 = Bytes.readString(bytes, 24, 7);
        LOGGER.info("value1 = [{}], value2 = [{}], value3 = [{}], value4 = [{}]", value1, value2, value3, value4);
    }

}
