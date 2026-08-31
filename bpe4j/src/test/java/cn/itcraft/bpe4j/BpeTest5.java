package cn.itcraft.bpe4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

/**
 * Async delivery mode acceptance test: callbacks must run on the engine's
 * egress thread (not the ingest thread), receive correct data, and the drop
 * counter must be readable.
 */
public class BpeTest5 {
    private static final Logger LOGGER = LoggerFactory.getLogger(BpeTest5.class);

    private static final String SQL = "select demo5.a, demo5.b, demo5.c, demo5.d from demo5 where demo5.a > 10 limit 10";

    @Test
    public void test() throws Exception {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaBpe.start();
        JavaBpe.setDeliveryMode(JavaBpe.DELIVERY_ASYNC);

        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        int recordId = JavaBpe.defIncoming("demo5", list);
        assertTrue("defIncoming failed", recordId != -1);

        CountDownLatch latch = new CountDownLatch(1);
        AtomicInteger total = new AtomicInteger();
        AtomicReference<String> threadName = new AtomicReference<>();
        AtomicReference<Long> firstA = new AtomicReference<>();

        int mapperId = JavaBpe.defMapper(SQL, (data, size) -> {
            // runs on the bpe-egress thread in async mode
            threadName.set(Thread.currentThread().getName());
            for (int i = 0; i < size; i++) {
                SimpleData d = converter.convert(data, i * 512);
                if (firstA.get() == null) {
                    firstA.set((long) d.getVal1());
                }
                LOGGER.info("async: {}", d);
            }
            total.addAndGet(size);
            latch.countDown();
        });
        assertTrue("defMapper failed", mapperId != -1);

        JavaBpe.regConvert(recordId, converter);
        for (int i = 0; i < 100; i++) {
            JavaBpe.newDataSync(recordId, new SimpleData(i, i + 1, i + 0.2D, Integer.toHexString(i)));
        }

        boolean delivered = latch.await(5, TimeUnit.SECONDS);
        assertTrue("async callback not delivered within 5s", delivered);
        // the egress thread attaches to the JVM unnamed (Thread-N); assert it is
        // neither the ingest thread nor the JVM dispatch thread
        String cbThread = threadName.get();
        assertTrue("callback must not run on the ingest thread: " + cbThread,
                !"main".equals(cbThread) && !"java-bpe-thread".equals(cbThread));
        assertEquals("first matched record is a=11", Long.valueOf(11), firstA.get());
        // wait until the event stream settles (all 100 records delivered)
        int prev = -1;
        for (int i = 0; i < 50 && total.get() != prev; i++) {
            prev = total.get();
            Thread.sleep(100);
        }
        LOGGER.info("async total delivered records: {}", total.get());
        assertTrue("delivered record count", total.get() >= 10);
        assertTrue("droppedEvents must be readable", JavaBpe.droppedEvents() >= 0);

        JavaBpe.setDeliveryMode(JavaBpe.DELIVERY_SYNC);
        JavaBpe.stop();
        LOGGER.info("async delivery test passed, dropped={}", JavaBpe.droppedEvents());
    }
}
