package com.erayt.bbpe4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;

/**
 * @author Helly Guo
 * <p>
 * Created on 10/25/23 2:10 PM
 */
public class BbpeTest4 {
    private static final Logger LOGGER = LoggerFactory.getLogger(BbpeTest4.class);

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";

    @Test
    public void test() {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaBbpe.start();
        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        int recordId = JavaBbpe.defIncoming("demo", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int mapperId = JavaBbpe.defMapper(SQL, (data, size) -> {
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
        JavaBbpe.regConvert(recordId, converter);
        @SuppressWarnings("rawtypes")
        CompletableFuture[] futures = new CompletableFuture[100];
        for (int i = 0; i < 100; i++) {
            futures[i] = JavaBbpe.newDataAsync(recordId,
                    new SimpleData(i, i + 1, i + 0.2D, Integer.toHexString(i)));
        }
        CompletableFuture.allOf(futures).join();
        JavaBbpe.stop();
    }

}
