#!/bin/bash
{
  printf "*1\r\n\$5\r\nMULTI\r\n"
  printf "*2\r\n\$5\r\nWATCH\r\n\$9\r\npineapple\r\n"
} | nc localhost 6379
