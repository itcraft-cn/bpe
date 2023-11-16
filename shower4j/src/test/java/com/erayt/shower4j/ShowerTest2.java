package com.erayt.shower4j;

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
public class ShowerTest2 {
    private static final Logger LOGGER = LoggerFactory.getLogger(ShowerTest2.class);

    private static final String SQL = "select demo.a, demo.b, demo.c, demo.d from demo limit 10";
    private static final String SQL2 = "select _suml(stream.a) from stream";

    @Test
    public void test() {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaShower.start();
        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        List<ColumnDefine> list2 = new ArrayList<>();
        list2.add(ColumnDefine.createLong("a"));
        list2.add(ColumnDefine.createLong("b"));
        list2.add(ColumnDefine.createDouble("c"));
        list2.add(ColumnDefine.createString("d", 16));
        int recordId = JavaShower.defIncoming("demo", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int recordId2 = JavaShower.defStream("stream", list2);
        if (recordId2 == -1) {
            LOGGER.warn("failed to def record");
            return;
        }
        int aggregateId = JavaShower.defAggregate(SQL2, (data, size) -> {
            for (int i = 0; i < size; i++) {
                SimpleData simpleData = converter.convert(data, i * 512);
                LOGGER.info("{}", simpleData);
            }
            LOGGER.info("size={}", size);
        });
        if (aggregateId == -1) {
            LOGGER.warn("failed to def aggregate");
            return;
        }
        int mapperId = JavaShower.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            return;
        }
        JavaShower.regConvert(recordId, converter);
        for (int i = 0; i < 100; i++) {
            JavaShower.newData(recordId, new SimpleData(i, i + 1, i + 0.2D, Integer.toHexString(i)));
        }
        JavaShower.stop();
    }

}
