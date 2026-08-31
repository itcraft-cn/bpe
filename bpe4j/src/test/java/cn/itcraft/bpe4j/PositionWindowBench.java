package cn.itcraft.bpe4j;

import org.openjdk.jmh.annotations.*;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Simulated business scenario: position accumulation.
 *
 * Trades flow into BPE (buy = +qty, sell = -qty); a 3s tumbling window
 * (processing time) aggregates the net open position; whenever the window
 * result exceeds 1,000,000 the Java-side disposal callback fires
 * (executed on the egress thread, never blocking ingest).
 *
 * One benchmark op = one trade submitted and acknowledged
 * (newDataAsync(...).get(): queue hand-off + JNI + engine insert).
 */
@BenchmarkMode(Mode.Throughput)
@OutputTimeUnit(TimeUnit.SECONDS)
@State(Scope.Benchmark)
@Fork(1)
@Warmup(iterations = 2, time = 1)
@Measurement(iterations = 3, time = 1)
public class PositionWindowBench {

    private static final long DISPOSE_THRESHOLD = 1_000_000L;
    private static final long WINDOW_MS = 3_000L;

    private int recordId;
    private final Trade trade = new Trade();
    private long lcg;
    /** trades acknowledged (engine-side) */
    private final AtomicLong trades = new AtomicLong();
    /** windows whose net position exceeded the threshold (Java-side disposal) */
    private final AtomicLong alerts = new AtomicLong();
    /** latest window net position, for the report */
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
        String sql = "select _suml(benchtrade.signed_qty) from benchtrade";
        int aggId = JavaBpe.defWindowAggregate(sql,
                JavaBpe.WINDOW_TUMBLING, WINDOW_MS, 0, 0, null, 0,
                (data, size) -> {
                    if (size >= 1) {
                        long net = ByteBuffer.wrap(data, 0, 8).order(ByteOrder.LITTLE_ENDIAN).getLong(0);
                        lastNet = net;
                        if (net > DISPOSE_THRESHOLD) {
                            // simulated disposal: must stay light (runs on the egress thread);
                            // real disposals (reduce-order, alerting) go here
                            alerts.incrementAndGet();
                        }
                    }
                });
        if (aggId == -1) {
            throw new IllegalStateException("defWindowAggregate failed");
        }
        lcg = 42;
    }

    @Benchmark
    public boolean trade() throws Exception {
        long qty = nextQty();
        trade.ts = System.currentTimeMillis();
        trade.signedQty = qty;
        boolean ok = JavaBpe.newDataAsync(recordId, trade).get(5, TimeUnit.SECONDS);
        if (ok) {
            trades.incrementAndGet();
        }
        return ok;
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
        System.out.println("==== position accumulation scenario ====");
        System.out.println("trades acked        : " + trades.get());
        System.out.println("windows fired       : (3s tumbling, processing time)");
        System.out.println("dispose alerts      : " + alerts.get() + " (net > " + DISPOSE_THRESHOLD + ")");
        System.out.println("last net position   : " + lastNet);
        System.out.println("egress pending      : " + JavaBpe.pendingEvents());
        System.out.println("egress dropped      : " + JavaBpe.droppedEvents());
        System.out.println("egress alerts(wd)   : " + JavaBpe.alertCount());
        JavaBpe.stop();
    }

    /** Mutable trade record (single-threaded: benchmark thread writes before ack). */
    static final class Trade {
        long ts;
        long signedQty;
    }

    /** Packs a Trade into the 512-byte record slot: [ts i64 @0][signed_qty i64 @8]. */
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
