from abc import abstractmethod, ABCMeta
from concurrent.futures import Future
from threading import Thread
from queue import Empty, Queue
from time import sleep
import bbpe4py


class BbpeRecordCallback(metaclass=ABCMeta):
    """Abstract class for Bbpe record callback."""

    @abstractmethod
    def callback(self, data: bytes) -> None:
        """callback. 回调

        Args:
            data(long[]): 数据.
        """


class WrappedData:
    def __init__(self, id: int, data: bytes) -> None:
        self._future = Future()
        self._id = id
        self._data = data

    def future(self) -> Future:
        return self._future

    def id(self) -> int:
        return self._id

    def data(self) -> bytes:
        return self._data


class Bbpe:
    """Bbpe client."""

    _bbpe_thread: Thread
    _data_queue: Queue = Queue(maxsize=10240)
    _active: bool = True

    @staticmethod
    def _run_thread() -> None:
        while Bbpe._active:
            try:
                data = Bbpe._data_queue.get(timeout=1)
            except Empty:
                sleep(0.01)
            else:
                result = Bbpe.__new_data(data.id(), data.data())
                data.future().set_result(result)

    @staticmethod
    def _fill_queue(data: WrappedData) -> None:
        Bbpe._data_queue.put(data)

    @staticmethod
    def start() -> bool:
        """Start Bbpe client. 启动

        Returns:
            bool: True if start success. 是否成功
        """
        Bbpe._bbpe_thread = Thread(target=Bbpe._run_thread)
        Bbpe._bbpe_thread.name = "py-bbpe-thread"
        Bbpe._bbpe_thread.daemon = True
        Bbpe._bbpe_thread.start()
        return bbpe4py.start()

    @staticmethod
    def stop() -> bool:
        """Stop Bbpe client. 停止

        Returns:
            bool: True if stop success. 是否成功
        """
        Bbpe._active = False
        Bbpe._bbpe_thread.join()
        return bbpe4py.stop()

    @staticmethod
    def def_incoming(name: str, names: list, types: list, lengths: list) -> int:
        """Define incoming var name. 通过 name 定义输入

        Args:
            name (str): name. 名称
            names (list): names. 名称列表
            types (list): types. 类型列表
            lengths (list): lengths. 长度列表

        Returns:
            int: id. 标号
        """
        return bbpe4py.def_incoming(name, names, types, lengths)

    @staticmethod
    def def_stream(name: str, names: list, types: list, lengths: list) -> int:
        """Define stream var name. 通过 name 定义流

        Args:
            name (str): name. 名称
            names (list): names. 名称列表
            types (list): types. 类型列表
            lengths (list): lengths. 长度列表

        Returns:
            int: id. 标号
        """
        return bbpe4py.def_stream(name, names, types, lengths)

    @staticmethod
    def new_data_sync(id: int, bdata: bytes) -> bool:
        """Create table. 新建数据表

        Args:
            id (int): id. 标号
            bdata (bytes): data. 数据

        Returns:
            bool: True if create success. 是否成功
        """
        return Bbpe.new_data_async(id, bdata).result()

    @staticmethod
    def new_data_async(id: int, bdata: bytes) -> Future:
        """Create table. 新建数据表

        Args:
            id (int): id. 标号
            bdata (bytes): data. 数据

        Returns:
            Future: future. 异步结果
        """
        data = WrappedData(id, bdata)
        Bbpe._fill_queue(data)
        return data.future()

    @staticmethod
    def __new_data(id: int, bdata: bytes) -> bool:
        """Create table. 新建数据表

        Args:
            id (int): id. 标号
            bdata (bytes): data. 数据

        Returns:
            bool: True if create success. 是否成功
        """
        return bbpe4py.new_data(id, bdata)

    @staticmethod
    def def_mapper(sql: str, callback: BbpeRecordCallback) -> int:
        """Define mapper var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (BbpeRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return bbpe4py.def_mapper(sql, callback)

    @staticmethod
    def def_mapper_bind_aggregate(sql: str, aggregate_id: int) -> int:
        """Define mapper var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (BbpeRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return bbpe4py.def_mapper_bind_aggregate(sql, aggregate_id)

    @staticmethod
    def def_aggregate(sql: str, callback: BbpeRecordCallback) -> int:
        """Define aggregate var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (BbpeRecordCallback): callback function. 回调函数

        Returns:
            int: >=0 if define success. 是否成功
        """
        return bbpe4py.def_aggregate(sql, callback)
