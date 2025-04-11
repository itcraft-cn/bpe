package com.erayt.bambootube4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
public class BambooTubeTest3 {
    private static final Logger LOGGER = LoggerFactory.getLogger(BambooTubeTest3.class);

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";

    @Test
    public void test() {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaBambooTube.start();
        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        int recordId = JavaBambooTube.defIncoming("demo", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int mapperId = JavaBambooTube.defMapper(SQL, (data, size) -> {
            for (int i = 0; i < size; i++) {
                SimpleData simpleData = converter.convert(data, i * 512);
                LOGGER.info("{}", simpleData);
            }
            LOGGER.info("size={}", size);
        });
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            return;
        }
        JavaBambooTube.regConvert(recordId, converter);
        for (int i = 0; i < 100; i++) {
            JavaBambooTube.newDataSync(recordId, new SimpleData(i, i + 1, i + 0.2D, Integer.toHexString(i)));
        }
        JavaBambooTube.stop();
    }

}
