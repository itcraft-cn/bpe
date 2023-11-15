package com.erayt.shower4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
public class ShowerTest2 {
    private static final Logger LOGGER = LoggerFactory.getLogger(ShowerTest2.class);
    private static final String SQL = "select demo.a from demo limit 10";
    private static final String SQL2 = "select _suml(stream.a) from stream";

    @Test
    public void test() {
        Shower.start();
        int recordId = Shower.defIncoming("demo", new String[]{"a"}, new int[]{0}, new int[]{0});
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int recordId2 = Shower.defStream("stream", new String[]{"a"}, new int[]{0}, new int[]{0});
        if (recordId2 == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int aggregateId = Shower.defAggregate(SQL2, (data, size) -> LOGGER.info("{}, {}", data, size));
        int mapperId = Shower.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            return;
        }
        for (int i = 0; i < 100; i++) {
            byte b = (byte) (i % 256);
            Shower.newData(1,
                    new byte[]{
                            b, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                            0, 0, 0, 0, 0, 0, 0, 0,
                    });
        }
        Shower.stop();
    }
}
