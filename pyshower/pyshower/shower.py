from abc import abstractmethod, ABCMeta
import shower4py


class ShowerRecordCallback(metaclass=ABCMeta):
    """Abstract class for Shower record callback."""

    @abstractmethod
    def callback(self, data) -> None:
        """callback. 回调

        Args:
            data(long[]): 数据.
        """


class Shower:
    """Shower client."""

    @staticmethod
    def start() -> bool:
        """Start Shower client. 启动

        Returns:
            bool: True if start success. 是否成功
        """
        return shower4py.start()

    @staticmethod
    def stop() -> bool:
        """Stop Shower client. 停止

        Returns:
            bool: True if stop success. 是否成功
        """
        return shower4py.stop()

    @staticmethod
    def new_data(id: int, bdata: bytes) -> bool:
        """Create table. 新建数据表

        Args:
            id (int): id. 标号
            bdata (bytes): data. 数据

        Returns:
            bool: True if create success. 是否成功
        """
        return shower4py.new_data(id, bdata)

    @staticmethod
    def def_action(sql:str)-> bool:
        """Define action var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。

        Returns:
            bool: True if define success. 是否成功
        """
        return shower4py.def_action(sql)

    @staticmethod
    def def_action_with_callback(sql:str, callback:ShowerRecordCallback)-> bool:
        """Define action var sql. 通过 sql 定义动作

        Args:
            sql (str): sql statement. sql 语句。
            callback (ShowerRecordCallback): callback function. 回调函数

        Returns:
            bool: True if define success. 是否成功
        """
        return shower4py.def_action_with_callback(sql, callback)
