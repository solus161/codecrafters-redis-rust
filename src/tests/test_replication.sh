#!/bin/bash
{
  printf "*2\r\n\$4\r\ninfo\r\n\$11\r\nreplication\r\n"

} | nc localhost 6379
