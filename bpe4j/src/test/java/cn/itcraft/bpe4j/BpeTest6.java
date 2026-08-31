package cn.itcraft.bpe4j;

import org.junit.Test;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.Assert.assertTrue;

/**
 * Egress watchdog acceptance test: a slow callback must raise an alert through
 * the injected Java listener with the documented 4x i64 payload.
 */
public class BpeTest6 {
    private static final Logger LOGGER = LoggerFactory.getLogger(BpeTest6.class);

    @Test
    public void test() throws Exception {
        SimpleDataConverter converter = new SimpleDataConverter();
        JavaBpe.start();

        CountDownLatch alertLatch = new CountDownLatch(1);
        AtomicLong elapsedMs = new AtomicLong(-1);
        AtomicLong policySeen = new AtomicLong(-1);
        AtomicLong alertThreadStamp = new AtomicLong(-1);

        // policy 3 = AlertOnly, threshold 80ms; the mapper callback sleeps 400ms
        JavaBpe.setEgressPolicy(JavaBpe.POLICY_ALERT_ONLY, 80);
        JavaBpe.setAlertListener((data, size) -> {
            if (size >= 1) {
                ByteBuffer buf = ByteBuffer.wrap(data, 0, 32).order(ByteOrder.LITTLE_ENDIAN);
                elapsedMs.set(buf.getLong(0));
                policySeen.set(buf.getLong(24));
                alertThreadStamp.set(System.currentTimeMillis());
                alertLatch.countDown();
            }
        });

        List<ColumnDefine> list = new ArrayList<>();
        list.add(ColumnDefine.createLong("a"));
        list.add(ColumnDefine.createLong("b"));
        list.add(ColumnDefine.createDouble("c"));
        list.add(ColumnDefine.createString("d", 16));
        int recordId = JavaBpe.defIncoming("demo6", list);
        assertTrue("defIncoming failed", recordId != -1);

        int mapperId = JavaBpe.defMapper("select demo6.a from demo6 where demo6.a > 0 limit 10", (data, size) -> {
            try {
                Thread.sleep(400); // slow callback: must trip the 80ms watchdog
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        });
        assertTrue("defMapper failed", mapperId != -1);

        JavaBpe.regConvert(recordId, converter);
        JavaBpe.newDataAsync(recordId, new SimpleData(1, 2, 0.5D, "x"))
                .get(5, TimeUnit.SECONDS);

        boolean alerted = alertLatch.await(5, TimeUnit.SECONDS);
        assertTrue("alert listener not invoked within 5s", alerted);
        assertTrue("reported elapsed must cover the threshold, got " + elapsedMs.get(),
                elapsedMs.get() >= 80);
        assertTrue("policy byte must be ALERT_ONLY(3), got " + policySeen.get(),
                policySeen.get() == JavaBpe.POLICY_ALERT_ONLY);

        // restore defaults and clear the listener
        JavaBpe.setAlertListener(null);
        JavaBpe.setEgressPolicy(JavaBpe.POLICY_ALERT_ONLY, 100);
        JavaBpe.stop();
        LOGGER.info("watchdog alert test passed: elapsedMs={} alerts={}",
                elapsedMs.get(), JavaBpe.alertCount());
    }
}
