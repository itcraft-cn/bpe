package cn.itcraft.bpe4j;

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
public class BpeTest2 {
    private static final Logger LOGGER = LoggerFactory.getLogger(BpeTest2.class);

    private static final String SQL = "select demo2.a, demo2.b, demo2.c, demo2.d from demo2 limit 10";
    private static final String SQL2 = "select _suml(stream.a), _suml(stream.b), _sumd(stream.c) from stream2";

    @Test
    public void test() {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaBpe.start();
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
        int recordId = JavaBpe.defIncoming("demo2", list);
        if (recordId == -1) {
            LOGGER.warn("failed to def record");
            return;
        } else {
            LOGGER.info("recordId={}", recordId);
        }
        int recordId2 = JavaBpe.defStream("stream2", list2);
        if (recordId2 == -1) {
            LOGGER.warn("failed to def record");
            return;
        } else {
            LOGGER.info("recordId={}", recordId2);
        }
        int aggregateId = JavaBpe.defAggregate(SQL2, (data, size) -> {
            for (int i = 0; i < size; i++) {
                SimpleData simpleData = converter.convert(data, i * 512);
                LOGGER.info("{}", simpleData);
            }
            LOGGER.info("size={}", size);
        });
        if (aggregateId == -1) {
            LOGGER.warn("failed to def aggregate");
            return;
        } else {
            LOGGER.info("aggregateId={}", aggregateId);
        }
        int mapperId = JavaBpe.defMapperBindAggregate(SQL, aggregateId);
        if (mapperId == -1) {
            LOGGER.warn("failed to def mapper");
            return;
        } else {
            LOGGER.info("mapperId={}", mapperId);
        }
        JavaBpe.regConvert(recordId, converter);
        for (int i = 0; i < 100; i++) {
            JavaBpe.newData(recordId, newData(i));
        }
        JavaBpe.stop();
    }

    private SimpleData newData(int i) {
        return new SimpleData(i, i + 1, i + 0.2D, Integer.toHexString(i));
    }

}
