package cn.itcraft.bpe4j;

import org.openjdk.jmh.annotations.*;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.atomic.LongAdder;

/**
 * Simulated business scenario: position accumulation.
 *
 * Trades flow into BPE (buy = +qty, sell = -qty); a 3s tumbling window
 * (processing time) aggregates the net open position and the record count;
 * whenever the window net position exceeds 1,000,000 the Java-side disposal
 * fires (on the egress thread, never blocking ingest).
 *
 * Throughput accounting (end-to-end "processed" semantics):
 * - each invocation fires OPS trades without waiting for per-trade acks;
 * - each window callback adds the window's record count to a LongAdder;
 * - the invocation returns only after the engine has processed all OPS
 *   trades (confirmed via the window callback counter);
 * - every REPORT_THRESHOLD processed trades, one report line is printed.
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@State(Scope.Benchmark)
@Fork(1)
@Warmup(iterations = 2, time = 8)
@Measurement(iterations = 4, time = 8)
public class PositionWindowBench {

    private static final long DISPOSE_THRESHOLD = 1_000_000L;
    private static final long WINDOW_MS = 3_000L;
    private static final int OPS = 100_000;
    private static final long REPORT_THRESHOLD = 100_000L;
    /** max wait for the window to deliver the last batch (window period + slack) */
    private static final long AWAIT_TIMEOUT_MS = 30_000L;

    private int recordId;
    private long lcg;
    /** trades submitted (fire-and-forget) */
    private final AtomicLong submitted = new AtomicLong();
    /** trades processed, confirmed by window callbacks (the scoring basis) */
    private final LongAdder processed = new LongAdder();
    private volatile long nextReport = REPORT_THRESHOLD;
    /** windows whose net position exceeded the threshold (Java-side disposal) */
    private final AtomicLong alerts = new AtomicLong();
    private volatile long lastNet;

    @Setup
    public void setup() {
        JavaBpe.start();
        List<ColumnDefine> schema = new ArrayList<>();
        schema.add(ColumnDefine.createLong("ts"));
        schema.add(ColumnDefine.createLong("signed_qty"));
        recordId = JavaBpe.defIncoming("benchtrade", schema);
        if (recordId == -1) {
            throw new IllegalStateException("defIncoming failed");
        }
        JavaBpe.regConvert(recordId, new TradeConverter());
        String sql = "select _suml(benchtrade.signed_qty), _count(benchtrade.signed_qty) from benchtrade";
        int aggId = JavaBpe.defWindowAggregate(sql,
                JavaBpe.WINDOW_TUMBLING, WINDOW_MS, 0, 0, null, 0,
                (data, size) -> {
                    if (size >= 1) {
                        ByteBuffer buf = ByteBuffer.wrap(data, 0, 16).order(ByteOrder.LITTLE_ENDIAN);
                        long net = buf.getLong(0);
                        long cnt = buf.getLong(8);
                        lastNet = net;
                        processed.add(cnt);
                        if (net > DISPOSE_THRESHOLD) {
                            // simulated disposal: must stay light (runs on the egress thread);
                            // real disposals (reduce-order, alerting) go here
                            alerts.incrementAndGet();
                        }
                        long done = processed.sum();
                        if (done >= nextReport) {
                            System.out.printf("[report] processed=%d submitted=%d alerts=%d lastNet=%d%n",
                                    done, submitted.get(), alerts.get(), net);
                            nextReport += REPORT_THRESHOLD;
                        }
                    }
                });
        if (aggId == -1) {
            throw new IllegalStateException("defWindowAggregate failed");
        }
        lcg = 42;
    }

    /**
     * One invocation = OPS trades: fire-and-forget submission, then wait until
     * the engine confirms all of them processed via the window callback counter.
     */
    @Benchmark
    @OperationsPerInvocation(OPS)
    public void trades() throws Exception {
        final long target = submitted.get() + OPS;
        for (int i = 0; i < OPS; i++) {
            Trade t = new Trade();
            t.ts = System.currentTimeMillis();
            t.signedQty = nextQty();
            // fire-and-forget: no per-trade ack wait; completion is accounted
            // by the window callback on the egress thread
            JavaBpe.newDataAsync(recordId, t);
            submitted.incrementAndGet();
        }
        awaitProcessed(target);
    }

    private void awaitProcessed(long target) throws InterruptedException {
        long deadline = System.currentTimeMillis() + AWAIT_TIMEOUT_MS;
        while (processed.sum() < target) {
            if (System.currentTimeMillis() > deadline) {
                throw new IllegalStateException(
                        "processing timeout: processed=" + processed.sum() + " target=" + target);
            }
            Thread.sleep(1);
        }
    }

    /** Deterministic pseudo-random net-long flow: +100k..+500k (always long). */
    private long nextQty() {
        lcg = lcg * 6364136223846793005L + 1442695040888963407L;
        return 100_000L + ((lcg >>> 33) % 400_000L);
    }

    @TearDown
    public void tearDown() throws InterruptedException {
        // wait one final window so the in-flight position is delivered
        Thread.sleep(WINDOW_MS + 500);
        System.out.println();
        System.out.println("==== position accumulation scenario (processed-accounting) ====");
        System.out.println("trades submitted   : " + submitted.get());
        System.out.println("trades processed   : " + processed.sum() + " (window-callback confirmed)");
        System.out.println("dispose alerts     : " + alerts.get() + " (net > " + DISPOSE_THRESHOLD + ")");
        System.out.println("last net position  : " + lastNet);
        System.out.println("egress pending     : " + JavaBpe.pendingEvents());
        System.out.println("egress dropped     : " + JavaBpe.droppedEvents());
        System.out.println("egress alerts(wd)  : " + JavaBpe.alertCount());
        JavaBpe.stop();
    }

    /** Trade record: [ts i64 @0][signed_qty i64 @8] in the 512-byte slot. */
    static final class Trade {
        long ts;
        long signedQty;
    }

    static final class TradeConverter implements ByteConverter<Trade> {
        @Override
        public byte[] convert(Trade data) {
            byte[] rec = new byte[512];
            ByteBuffer buf = ByteBuffer.wrap(rec).order(ByteOrder.LITTLE_ENDIAN);
            buf.putLong(0, data.ts);
            buf.putLong(8, data.signedQty);
            return rec;
        }

        @Override
        public Trade convert(byte[] data, int offset) {
            Trade t = new Trade();
            ByteBuffer buf = ByteBuffer.wrap(data, offset, 16).order(ByteOrder.LITTLE_ENDIAN);
            t.ts = buf.getLong(0);
            t.signedQty = buf.getLong(8);
            return t;
        }
    }
}
